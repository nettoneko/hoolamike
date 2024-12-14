use {
    super::{concurrency, DownloadSummary},
    crate::{
        compression::{ArchiveHandle, ProcessArchive, SeekWithTempFileExt},
        downloaders::helpers::FutureAnyhowExt,
        modlist_json::directive::ArchiveHashPath,
        progress_bars::{vertical_progress_bar, ProgressKind},
    },
    anyhow::{Context, Result},
    futures::{FutureExt, TryFutureExt},
    indexmap::IndexMap,
    indicatif::ProgressBar,
    once_cell::sync::Lazy,
    std::{
        collections::BTreeMap,
        convert::identity,
        future::ready,
        io::Seek,
        path::{Path, PathBuf},
        sync::Arc,
    },
    tap::prelude::*,
    tempfile::{NamedTempFile, SpooledTempFile},
    tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, SemaphorePermit},
};

impl ArchiveHashPath {
    pub fn parent(self) -> Option<(Self, crate::utils::MaybeWindowsPath)> {
        self.pipe(|Self { source_hash, mut path }| {
            path.pop()
                .map(|popped| (Self { source_hash, path }, popped))
        })
    }
}

#[derive(derivative::Derivative)]
#[derivative(Debug(bound = ""))]
pub struct NestedArchivesService {
    pub download_summary: DownloadSummary,
    pub max_size: usize,
    #[derivative(Debug = "ignore")]
    pub cache: IndexMap<ArchiveHashPath, CachedArchiveFile>,
}

impl NestedArchivesService {
    pub fn new(download_summary: DownloadSummary, max_size: usize) -> Self {
        Self {
            max_size,
            download_summary,
            cache: Default::default(),
        }
    }
}

pub fn max_open_files() -> usize {
    concurrency() * 4
}
pub(crate) static OPEN_FILE_PERMITS: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(max_open_files())));

pub struct WithPermit<T> {
    pub permit: OwnedSemaphorePermit,
    pub inner: T,
}

impl<T> WithPermit<T>
where
    T: Send + 'static,
{
    pub async fn new<Fut, F>(semaphore: &Arc<Semaphore>, new: F) -> Result<Self>
    where
        Fut: std::future::Future<Output = Result<T>>,
        F: FnOnce() -> Fut,
    {
        semaphore
            .clone()
            .acquire_owned()
            .map_context("semaphore closed")
            .and_then(move |permit| new().map_ok(|inner| Self { permit, inner }))
            .await
    }
    pub async fn new_blocking<F>(semaphore: &Arc<Semaphore>, new: F) -> Result<Self>
    where
        F: FnOnce() -> Result<T> + Clone + Send + 'static,
    {
        Self::new(semaphore, || {
            tokio::task::spawn_blocking(new)
                .map_context("thread crashed")
                .and_then(ready)
        })
        .await
    }
}

pub type CachedArchiveFile = Arc<WithPermit<(NamedTempFile, std::fs::File)>>;
pub enum HandleKind {
    Cached(CachedArchiveFile),
    JustHashPath(PathBuf),
}

impl NestedArchivesService {
    #[tracing::instrument(skip_all)]
    async fn init(&mut self, archive_hash_path: ArchiveHashPath) -> Result<(ArchiveHashPath, HandleKind)> {
        let pb = vertical_progress_bar(0, ProgressKind::ExtractTemporaryFile, indicatif::ProgressFinish::AndClear)
            .attach_to(&super::PROGRESS_BAR)
            .tap_mut(|pb| {
                pb.set_message(
                    archive_hash_path
                        .pipe_ref(serde_json::to_string)
                        .expect("must serialize"),
                );
            });
        async fn get_handle(pb: ProgressBar, file: std::fs::File, path: PathBuf, archive_path: PathBuf) -> Result<CachedArchiveFile> {
            tokio::task::spawn_blocking(move || {
                ArchiveHandle::guess(file, &path)
                    .context("could not guess archive format for [{path}]")
                    .and_then(|mut archive| archive.get_handle(&archive_path.clone()))
            })
            .map_context("thread crashed")
            .and_then(ready)
            .and_then(|handle| handle.seek_with_temp_file(pb))
            .await
        }
        match archive_hash_path.clone().parent() {
            Some((parent, archive_path)) => match self.get(parent).boxed_local().await? {
                HandleKind::Cached(cached) => {
                    cached
                        .inner
                        .1
                        .try_clone()
                        .context("cloning file handle")
                        .and_then(|mut cloned| cloned.rewind().context("rewinding file").map(|_| cloned))
                        .pipe(ready)
                        .and_then(|file| get_handle(pb, file, cached.inner.0.path().to_owned(), archive_path.into_path()))
                        .map_ok(HandleKind::Cached)
                        .await
                }
                HandleKind::JustHashPath(path_buf) => {
                    std::fs::OpenOptions::new()
                        .read(true)
                        .open(&path_buf)
                        .with_context(|| format!("opening [{path_buf:?}]"))
                        .pipe(ready)
                        .and_then(|file| get_handle(pb, file, path_buf, archive_path.into_path()))
                        .map_ok(HandleKind::Cached)
                        .await
                }
            },
            None => self
                .download_summary
                .get(&archive_hash_path.source_hash)
                .with_context(|| format!("could not find file by hash path: {:#?}", archive_hash_path))
                .map(|downloaded| downloaded.inner.clone())
                .map(HandleKind::JustHashPath),
        }
        .map(|handle| (archive_hash_path, handle))
    }
    #[tracing::instrument(skip(self))]
    pub async fn get(&mut self, nested_archive: ArchiveHashPath) -> Result<HandleKind> {
        match self.cache.get(&nested_archive) {
            Some(exists) => {
                // WARN: this is dirty but it prevents small files from piling up
                exists.clone().pipe(HandleKind::Cached).pipe(Ok)
            }
            None => {
                if self.cache.len() == self.max_size {
                    tracing::info!("dropping cached archive");
                    self.cache.shift_remove_index(0);
                }
                let (hash_path, handle) = self
                    .init(nested_archive)
                    .await
                    .context("initializing a new archive handle")?;
                if let HandleKind::Cached(cached) = &handle {
                    self.cache.insert(hash_path, cached.clone());
                }
                Ok(handle)
            }
        }
    }
}

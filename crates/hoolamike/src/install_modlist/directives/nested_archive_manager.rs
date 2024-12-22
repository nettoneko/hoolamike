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
    itertools::Itertools,
    once_cell::sync::Lazy,
    std::{
        future::ready,
        io::Seek,
        path::PathBuf,
        sync::Arc,
    },
    tap::prelude::*,
    tempfile::NamedTempFile,
    tokio::{
        sync::{Mutex, OwnedSemaphorePermit, Semaphore},
        time::Instant,
    },
    tracing::{info_span, instrument, Instrument},
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
    pub cache: IndexMap<ArchiveHashPath, (CachedArchiveFile, tokio::time::Instant)>,
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
    concurrency() * 20
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

pub type CachedArchiveFile = Arc<WithPermit<Mutex<(NamedTempFile, std::fs::File)>>>;
pub enum HandleKind {
    Cached(CachedArchiveFile),
    JustHashPath(PathBuf),
}

fn ancestors(archive_hash_path: ArchiveHashPath) -> impl Iterator<Item = ArchiveHashPath> {
    std::iter::successors(Some(archive_hash_path), |p| p.clone().parent().map(|(parent, _)| parent))
}

impl NestedArchivesService {
    #[instrument(skip(self))]
    async fn init(&mut self, archive_hash_path: ArchiveHashPath) -> Result<(ArchiveHashPath, HandleKind)> {
        tracing::trace!("initializing entry");
        let pb = vertical_progress_bar(0, ProgressKind::ExtractTemporaryFile, indicatif::ProgressFinish::AndClear)
            .attach_to(&super::PROGRESS_BAR)
            .tap_mut(|pb| {
                pb.set_message(
                    archive_hash_path
                        .pipe_ref(serde_json::to_string)
                        .expect("must serialize"),
                );
            });
        #[instrument(skip(pb, file), level = "INFO")]
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
                    let file = cached
                        .clone()
                        .inner
                        .lock()
                        .instrument(info_span!("waiting_for_cached_file_entry_lock_to_roll_it_back"))
                        .await
                        .1
                        .pipe_ref_mut(|file| {
                            file.rewind()
                                .context("rewinding file")
                                .and_then(|_| file.try_clone().context("cloning file handle"))
                        })?;

                    get_handle(
                        pb,
                        file,
                        cached
                            .inner
                            .lock()
                            .instrument(info_span!("waiting_for_cached_file_entry_lock_to_clone_it"))
                            .await
                            .0
                            .path()
                            .to_owned(),
                        archive_path.into_path(),
                    )
                    .await
                    .map(HandleKind::Cached)
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
                .tap_some(|path| tracing::debug!("translated [{}] => [{}]\n\n\n", archive_hash_path.source_hash, path.inner.display()))
                .with_context(|| format!("could not find file by hash path: {:#?}", archive_hash_path))
                .map(|downloaded| downloaded.inner.clone())
                .map(HandleKind::JustHashPath),
        }
        .map(|handle| (archive_hash_path, handle))
    }
    #[instrument(skip(self), level = "INFO")]
    pub async fn get(&mut self, nested_archive: ArchiveHashPath) -> Result<HandleKind> {
        match self.cache.get(&nested_archive).cloned() {
            Some((exists, _last_accessed)) => {
                // WARN: this is dirty but it prevents small files from piling up
                exists
                    .inner
                    .lock()
                    .await
                    .1
                    .rewind()
                    .context("rewinding file")?;
                let exists = exists.pipe(HandleKind::Cached);
                ancestors(nested_archive).for_each(|ancestor| {
                    let now = Instant::now();
                    if let Some((_, accessed)) = self.cache.get_mut(&ancestor) {
                        *accessed = now;
                    }
                });
                Ok(exists)
            }
            None => {
                if self.cache.len() == self.max_size {
                    tracing::info!("dropping cached archive");
                    if let Some(last_accessed_chunk) = self
                        .cache
                        .iter()
                        .sorted_unstable_by_key(|(_key, (_, accessed))| accessed)
                        .chunk_by(|(_, (_, accessed))| accessed)
                        .into_iter()
                        .next()
                        .map(|(_, k)| k.map(|(key, _)| key.clone()).collect_vec())
                        .into_iter()
                        .next()
                    {
                        last_accessed_chunk.into_iter().for_each(|key| {
                            self.cache.shift_remove(&key);
                        })
                    }
                }
                let (hash_path, handle) = self
                    .init(nested_archive)
                    .await
                    .context("initializing a new archive handle")?;
                if let HandleKind::Cached(cached) = &handle {
                    self.cache
                        .insert(hash_path, (cached.clone(), Instant::now()));
                }
                Ok(handle)
            }
        }
    }
    #[tracing::instrument(skip(self))]
    pub async fn preheat(&mut self, archive_hash_path: ArchiveHashPath) -> Result<()> {
        tracing::trace!("preheating");
        self.get(archive_hash_path).await.map(|_| ())
    }
    #[tracing::instrument(skip(self))]
    pub fn cleanup(&mut self, archive_hash_path: ArchiveHashPath) {
        tracing::trace!("preheating");
        ancestors(archive_hash_path).for_each(|ancestor| {
            self.cache.shift_remove(&ancestor);
        })
    }
}

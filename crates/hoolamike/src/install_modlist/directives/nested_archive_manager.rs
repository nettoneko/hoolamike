use {
    super::DownloadSummary,
    crate::{
        compression::{ArchiveHandle, ProcessArchive, SeekWithTempFileExt},
        modlist_json::directive::ArchiveHashPath,
        progress_bars::{vertical_progress_bar, ProgressKind},
    },
    anyhow::{Context, Result},
    futures::FutureExt,
    indexmap::IndexMap,
    indicatif::ProgressBar,
    std::{
        collections::BTreeMap,
        convert::identity,
        path::{Path, PathBuf},
        sync::Arc,
    },
    tap::prelude::*,
    tempfile::{NamedTempFile, SpooledTempFile},
    tokio::sync::Mutex,
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
    pub cache: IndexMap<ArchiveHashPath, Arc<CachedArchiveFile>>,
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

pub type CachedArchiveFile = (NamedTempFile, std::fs::File);
pub enum HandleKind {
    Cached(Arc<CachedArchiveFile>),
    JustHashPath(PathBuf),
}

impl NestedArchivesService {
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
        fn get_handle(pb: ProgressBar, file: std::fs::File, path: PathBuf, archive_path: PathBuf) -> Result<CachedArchiveFile> {
            ArchiveHandle::guess(file, &path)
                .context("could not guess archive format for [{path}]")
                .and_then(|mut archive| {
                    archive
                        .get_handle(&archive_path.clone())
                        .context("reading file out of an archive")
                        .and_then(|handle| handle.seek_with_temp_file(pb))
                })
        }
        match archive_hash_path.clone().parent() {
            Some((parent, archive_path)) => match self.get(parent).boxed_local().await? {
                HandleKind::Cached(cached) => tokio::task::spawn_blocking(move || {
                    cached
                        .1
                        .try_clone()
                        .context("cloning file handle")
                        .and_then(|file| get_handle(pb, file, cached.0.path().to_owned(), archive_path.into_path()))
                        .map(Arc::new)
                        .map(HandleKind::Cached)
                })
                .await
                .context("thread crashed")
                .and_then(identity),
                HandleKind::JustHashPath(path_buf) => tokio::task::spawn_blocking(move || {
                    std::fs::OpenOptions::new()
                        .read(true)
                        .open(&path_buf)
                        .with_context(|| format!("opening [{path_buf:?}]"))
                        .and_then(|file| get_handle(pb, file, path_buf, archive_path.into_path()))
                        .map(Arc::new)
                        .map(HandleKind::Cached)
                })
                .await
                .context("thread crashed")
                .and_then(identity),
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
    pub async fn get(&mut self, nested_archive: ArchiveHashPath) -> Result<HandleKind> {
        match self.cache.get(&nested_archive) {
            Some(exists) => {
                // WARN: this is dirty but it prevents small files from piling up
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                exists.clone().pipe(HandleKind::Cached).pipe(Ok)
            }
            None => {
                let (hash_path, handle) = self
                    .init(nested_archive)
                    .await
                    .context("initializing a new archive handle")?;
                if let HandleKind::Cached(cached) = &handle {
                    if self.cache.len() == self.max_size {
                        tracing::info!("dropping cached archive");
                        self.cache.shift_remove_index(0);
                    }
                    self.cache.entry(hash_path).or_insert(cached.clone());
                }
                Ok(handle)
            }
        }
    }
}

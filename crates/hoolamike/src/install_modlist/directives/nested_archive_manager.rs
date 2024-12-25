use {
    super::{concurrency, DownloadSummary},
    crate::{
        compression::{ArchiveHandle, ProcessArchive, SeekWithTempFileExt},
        downloaders::helpers::FutureAnyhowExt,
        modlist_json::directive::ArchiveHashPath,
        utils::PathReadWrite,
    },
    anyhow::{Context, Result},
    futures::TryFutureExt,
    once_cell::sync::Lazy,
    std::{
        future::ready,
        path::{Path, PathBuf},
        sync::Arc,
    },
    tap::prelude::*,
    tokio::{
        sync::{OwnedSemaphorePermit, Semaphore},
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

pub fn max_open_files() -> usize {
    concurrency() * 20
}
pub(crate) static OPEN_FILE_PERMITS: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(max_open_files())));

pub type CachedArchiveFile = Arc<WithPermit<tempfile::TempPath>>;
pub enum HandleKind {
    Cached(CachedArchiveFile),
    JustHashPath(PathBuf),
}

impl HandleKind {
    pub fn open_file_read(&self) -> Result<(PathBuf, std::fs::File)> {
        match self {
            HandleKind::Cached(cached) => cached.inner.open_file_read(),
            HandleKind::JustHashPath(path_buf) => path_buf.open_file_read(),
        }
    }
}

pub enum OutputHandleKind {
    FreshlyCreated(WithPermit<tempfile::TempPath>),
    JustHashPath(PathBuf),
}

impl OutputHandleKind {
    pub fn open_file_read(&self) -> Result<(PathBuf, std::fs::File)> {
        match self {
            OutputHandleKind::FreshlyCreated(cached) => cached.inner.open_file_read(),
            OutputHandleKind::JustHashPath(path_buf) => path_buf.open_file_read(),
        }
    }
}
fn ancestors(archive_hash_path: ArchiveHashPath) -> impl Iterator<Item = ArchiveHashPath> {
    std::iter::successors(Some(archive_hash_path), |p| p.clone().parent().map(|(parent, _)| parent))
}

#[derive(Debug)]
pub struct WithPermit<T> {
    pub permit: OwnedSemaphorePermit,
    pub inner: T,
}

impl<T> WithPermit<T>
where
    T: Send + 'static,
{
    #[instrument(skip_all, level = "DEBUG")]
    pub async fn new<Fut, F>(semaphore: &Arc<Semaphore>, new: F) -> Result<Self>
    where
        Fut: std::future::Future<Output = Result<T>>,
        F: FnOnce() -> Fut,
    {
        semaphore
            .clone()
            .acquire_owned()
            .instrument(info_span!("waiting_for_file_permit"))
            .map_context("semaphore closed")
            .and_then(move |permit| new().map_ok(|inner| Self { permit, inner }))
            .await
    }
    #[instrument(skip_all, level = "DEBUG")]
    pub async fn new_blocking<F>(semaphore: &Arc<Semaphore>, new: F) -> Result<Self>
    where
        F: FnOnce() -> Result<T> + Clone + Send + 'static,
    {
        let span = tracing::Span::current();
        Self::new(semaphore, move || {
            tokio::task::spawn_blocking(move || span.in_scope(new))
                .instrument(tracing::Span::current())
                .map_context("thread crashed")
                .and_then(ready)
        })
        .await
    }
}

pub struct NestedArchivesService(Arc<NestedArchivesServiceInner>);

impl NestedArchivesService {
    pub fn new(download_summary: DownloadSummary, max_size: usize) -> Self {
        NestedArchivesServiceInner::new(download_summary, max_size)
            .pipe(Arc::new)
            .pipe(Self)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get(self: Arc<Self>, nested_archive: ArchiveHashPath) -> Result<OutputHandleKind> {
        match nested_archive.clone().parent() {
            Some((parent, path)) => match parent.path.len() {
                0 => {
                    get_handle(&self.0.clone().translate_source_hash(&parent)?, &path.into_path())
                        .map_ok(OutputHandleKind::FreshlyCreated)
                        .await
                }
                _ => {
                    get_handle(
                        &self
                            .try_get(parent)
                            .await
                            .context("no entry in cache")?
                            .inner,
                        &path.into_path(),
                    )
                    .map_ok(OutputHandleKind::FreshlyCreated)
                    .await
                }
            },
            None => self
                .0
                .clone()
                .translate_source_hash(&nested_archive)
                .map(OutputHandleKind::JustHashPath),
        }
    }

    #[instrument(skip(self), level = "INFO")]
    async fn try_get(self: Arc<Self>, nested_archive: ArchiveHashPath) -> Option<CachedArchiveFile> {
        self.0
            .cache
            .get(&nested_archive)
            .as_deref()
            .map(|(e, _)| e.clone())
    }
    #[tracing::instrument(skip(self))]
    pub async fn preheat(self: Arc<Self>, archive_hash_path: ArchiveHashPath) -> Result<()> {
        self.0.clone().preheat(archive_hash_path).await
    }
    #[tracing::instrument(skip(self))]
    pub async fn cleanup(self: Arc<Self>, archive_hash_path: ArchiveHashPath) {
        self.0.clone().cleanup(archive_hash_path)
    }
}

#[instrument(level = "INFO")]
async fn get_handle(source_path: &Path, archive_path: &Path) -> Result<WithPermit<tempfile::TempPath>> {
    tokio::task::spawn_blocking({
        let (source_path, archive_path) = (source_path.to_owned(), archive_path.to_owned());
        move || {
            ArchiveHandle::guess(&source_path)
                .context("could not guess archive format for [{path}]")
                .and_then(|mut archive| archive.get_handle(&archive_path.clone()))
        }
    })
    .map_context("thread crashed")
    .and_then(ready)
    .and_then(|mut handle| {
        handle
            .size()
            .pipe(ready)
            .and_then(|size| handle.seek_with_temp_file(size))
    })
    .await
    .with_context(|| {
        format!(
            "when extracting path {} from within archive at {}",
            archive_path.display(),
            source_path.display(),
        )
    })
}

#[derive(derivative::Derivative)]
#[derivative(Debug(bound = ""))]
struct NestedArchivesServiceInner {
    download_summary: DownloadSummary,
    max_size: usize,
    #[derivative(Debug = "ignore")]
    cache: dashmap::DashMap<ArchiveHashPath, (CachedArchiveFile, tokio::time::Instant)>,
}

impl NestedArchivesServiceInner {
    fn new(download_summary: DownloadSummary, max_size: usize) -> Self {
        Self {
            max_size,
            download_summary,
            cache: Default::default(),
        }
    }
    fn translate_source_hash(self: Arc<Self>, archive_hash_path: &ArchiveHashPath) -> Result<PathBuf> {
        self.download_summary
            .get(&archive_hash_path.source_hash)
            .tap_some(|path| tracing::debug!("translated [{}] => [{}]\n\n\n", archive_hash_path.source_hash, path.inner.display()))
            .with_context(|| format!("could not find file by hash path: {:#?}", archive_hash_path))
            .map(|downloaded| downloaded.inner.clone())
    }
    #[instrument(skip(self), fields(file_permits=%OPEN_FILE_PERMITS.available_permits(), cache_entries_count=%self.cache.len()))]
    async fn init(self: Arc<Self>, archive_hash_path: ArchiveHashPath) -> Result<(ArchiveHashPath, HandleKind)> {
        tracing::trace!("initializing entry");
        match archive_hash_path.clone().parent() {
            Some((parent, archive_path)) => match self.get(parent).pipe(Box::pin).await? {
                HandleKind::Cached(cached) => {
                    get_handle(&cached.inner, &archive_path.into_path())
                        .map_ok(Arc::new)
                        .map_ok(HandleKind::Cached)
                        .await
                }
                HandleKind::JustHashPath(path_buf) => {
                    get_handle(&path_buf, &archive_path.into_path())
                        .map_ok(Arc::new)
                        .map_ok(HandleKind::Cached)
                        .await
                }
            },
            None => self
                .translate_source_hash(&archive_hash_path)
                .map(HandleKind::JustHashPath),
        }
        .map(|handle| (archive_hash_path, handle))
    }
    #[instrument(skip(self), level = "INFO")]
    async fn get(self: Arc<Self>, nested_archive: ArchiveHashPath) -> Result<HandleKind> {
        match self.cache.get(&nested_archive).as_deref().cloned() {
            Some((exists, _last_accessed)) => {
                let exists = exists.pipe(HandleKind::Cached);
                ancestors(nested_archive).for_each(|ancestor| {
                    let now = Instant::now();
                    if let Some((_, accessed)) = self.cache.get_mut(&ancestor).as_deref_mut() {
                        *accessed = now;
                    }
                });
                Ok(exists)
            }
            None => {
                let (hash_path, handle) = self
                    .clone()
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
    async fn preheat(self: Arc<Self>, archive_hash_path: ArchiveHashPath) -> Result<()> {
        tracing::trace!("preheating");
        self.get(archive_hash_path).await.map(|_| ())
    }
    #[tracing::instrument(skip(self))]
    fn cleanup(self: Arc<Self>, archive_hash_path: ArchiveHashPath) {
        tracing::trace!("preheating");
        ancestors(archive_hash_path).for_each(|ancestor| {
            self.cache.remove(&ancestor);
        })
    }
}

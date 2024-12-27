use {
    super::nested_archive_manager::WithPermit,
    crate::compression::{ArchiveHandle, ProcessArchive, SeekWithTempFileExt},
    futures::{FutureExt, TryFutureExt},
    nonempty::NonEmpty,
    std::{convert::identity, future::ready, path::PathBuf, sync::Arc},
    tap::prelude::*,
    tokio::{
        sync::{watch::error::RecvError, AcquireError, Semaphore},
        task::JoinHandle,
    },
    tracing::{debug, debug_span, instrument, Instrument},
};

#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    #[error("channel has been closed")]
    ChannelClosed(#[source] RecvError),
    #[error("extraction from archive failed")]
    ExtractingFromArchive(#[source] Arc<anyhow::Error>),
    #[error("thread crashed")]
    ThreadCrashed(#[source] Arc<tokio::task::JoinError>),
    #[error("could not acquire permit")]
    AcquiringPermit(#[source] Arc<AcquireError>),
}

impl Error {
    pub fn extracting_from_archive(error: anyhow::Error) -> Self {
        error.pipe(Arc::new).pipe(Self::ExtractingFromArchive)
    }
    pub fn thread_crashed(error: tokio::task::JoinError) -> Self {
        error.pipe(Arc::new).pipe(Self::ThreadCrashed)
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub type Extracted = WithPermit<tempfile::TempPath>;

#[derive(Debug)]
pub enum SourceKind {
    JustPath(PathBuf),
    CachedPath(Extracted),
}

pub mod cached_future;

#[derive(Clone)]
pub enum CacheState {
    InProgress(Arc<tokio::sync::Notify>),
    Ready(Result<Arc<SourceKind>>),
}

pub struct QueuedArchiveService {
    pub tasks: dashmap::DashMap<NonEmpty<PathBuf>, CacheState>,
    pub permits: Arc<Semaphore>,
}

impl QueuedArchiveService {
    pub fn new(concurrency: usize) -> Arc<Self> {
        Arc::new(Self {
            tasks: Default::default(),
            permits: Arc::new(Semaphore::new(concurrency)),
        })
    }

    pub fn get_archive_spawn(self: Arc<Self>, archive: NonEmpty<PathBuf>) -> JoinHandle<Result<Arc<SourceKind>>> {
        tokio::task::spawn(self.get_archive(archive))
    }

    #[instrument(skip(self))]
    pub async fn get_archive(self: Arc<Self>, archive: NonEmpty<PathBuf>) -> Result<Arc<SourceKind>> {
        match self.clone().tasks.entry(archive.clone()) {
            dashmap::Entry::Occupied(occupied_entry) => match occupied_entry.into_ref().clone() {
                CacheState::Ready(already_exists) => already_exists,
                CacheState::InProgress(waiting) => {
                    waiting
                        .notified()
                        .instrument(debug_span!("operation in progress, awaiting it's completion"))
                        .await;
                    self.clone()
                        .get_archive(archive.clone())
                        .instrument(debug_span!("getting another archive because got notified"))
                        .pipe(Box::pin)
                        .await
                }
            },
            dashmap::Entry::Vacant(vacant_entry) => {
                fn popped<T>(mut l: NonEmpty<T>) -> Option<(NonEmpty<T>, T)> {
                    l.pop().map(|i| (l, i))
                }

                let notify = tokio::sync::Notify::new().pipe(Arc::new);
                vacant_entry.insert(notify.clone().pipe(CacheState::InProgress));
                match popped(archive.clone()) {
                    Some((parent, archive_path)) => {
                        self.clone()
                            .get_archive(parent)
                            .instrument(debug_span!("entry was not found, so scheduling creation of parent"))
                            .pipe(Box::pin)
                            .and_then(|parent| {
                                prepare_archive(self.permits.clone(), parent, archive_path)
                                    .instrument(tracing::Span::current())
                                    .instrument(debug_span!("preparing archive"))
                            })
                            .map(|result| {
                                let result = result.map(SourceKind::CachedPath).map(Arc::new);
                                match self
                                    .tasks
                                    .insert(archive, result.clone().pipe(CacheState::Ready))
                                {
                                    Some(CacheState::InProgress(notify)) => {
                                        debug!("notifying of ready parent archive task");
                                        notify.notify_waiters()
                                    }
                                    Some(CacheState::Ready(already_exists)) => tracing::error!(?already_exists, "the work was duplicated?"),
                                    None => unreachable!("nobody is waiting for the task?"),
                                };
                                result
                            })
                            .await
                    }
                    None => Ok(Arc::new(SourceKind::JustPath(archive.head))),
                }
                .tap(|_finished| {
                    debug!("notifying of ready current archive task");
                    notify.notify_waiters();
                })
            }
        }
    }
}

#[instrument]
async fn prepare_archive(permits: Arc<Semaphore>, source: Arc<SourceKind>, archive_path: PathBuf) -> Result<Extracted> {
    let run = tracing::Span::current();
    tokio::task::spawn({
        cloned![run];
        async move {
            permits
                .acquire_owned()
                .instrument(debug_span!("acquiring file permit"))
                .map_err(Arc::new)
                .map_err(self::Error::AcquiringPermit)
                .map_ok(|permit| (source, permit))
                .and_then({
                    cloned![run];
                    move |(source, permit)| {
                        tokio::task::spawn_blocking(move || {
                            run.in_scope(|| {
                                ArchiveHandle::guess(source.as_ref().as_ref())
                                    .map_err(self::Error::extracting_from_archive)
                                    .and_then(|mut archive| {
                                        archive
                                            .get_handle(&archive_path)
                                            .map_err(self::Error::extracting_from_archive)
                                            .and_then(|mut handle| {
                                                handle
                                                    .size()
                                                    .and_then(|size| handle.seek_with_temp_file_blocking(size, permit))
                                                    .map_err(self::Error::extracting_from_archive)
                                            })
                                    })
                            })
                        })
                        .map_err(self::Error::thread_crashed)
                        .and_then(ready)
                    }
                })
                .instrument(run)
                .instrument(debug_span!("waiting for thread to finish"))
                .await
        }
    })
    .map_err(self::Error::thread_crashed)
    .and_then(ready)
    .instrument(run)
    .await
}

impl AsRef<std::path::Path> for SourceKind {
    fn as_ref(&self) -> &std::path::Path {
        match self {
            SourceKind::JustPath(path_buf) => path_buf,
            SourceKind::CachedPath(cached) => &cached.inner,
        }
    }
}

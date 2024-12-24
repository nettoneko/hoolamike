use {
    super::*,
    crate::{
        config_file::{DownloadersConfig, GamesConfig},
        downloaders::{
            gamefile_source_downloader::{get_game_file_source_synchronizers, GameFileSourceSynchronizers},
            helpers::FutureAnyhowExt,
            nexus::{self, NexusDownloader},
            wabbajack_cdn::WabbajackCDNDownloader,
            CopyFileTask,
            DownloadTask,
            MergeDownloadTask,
            SyncTask,
            WithArchiveDescriptor,
        },
        error::{MultiErrorCollectExt, TotalResult},
        modlist_json::{Archive, GoogleDriveState, HttpState, ManualState, NexusState, State},
        progress_bars_v2::IndicatifWrapIoExt,
    },
    anyhow::Result,
    futures::{FutureExt, StreamExt, TryStreamExt},
    std::{path::PathBuf, sync::Arc},
    tracing::{instrument, warn},
};

#[derive(Clone)]
pub struct DownloadersInner {
    pub nexus: Option<Arc<NexusDownloader>>,
}

impl DownloadersInner {
    pub fn new(DownloadersConfig { nexus, downloads_directory: _ }: DownloadersConfig) -> Result<Self> {
        Ok(Self {
            nexus: nexus
                .api_key
                .map(NexusDownloader::new)
                .transpose()?
                .map(Arc::new),
        })
    }
}

#[derive(Clone)]
pub struct Synchronizers {
    pub config: Arc<DownloadersConfig>,
    inner: DownloadersInner,
    pub(crate) cache: Arc<download_cache::DownloadCache>,
    game_synchronizers: Arc<GameFileSourceSynchronizers>,
}

enum Either<L, R> {
    Left(L),
    Right(R),
}

#[instrument]
async fn copy_local_file(from: PathBuf, to: PathBuf, expected_size: u64) -> Result<PathBuf> {
    let file_name = to
        .file_name()
        .expect("file must have a name")
        .to_string_lossy()
        .to_string();

    let mut source_file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(&from)
        .map_with_context(|| format!("opening [{}]", from.display()))
        .await?;
    let target_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&to)
        .map_with_context(|| format!("opening [{}]", to.display()))
        .await?;

    let copied = tokio::io::copy(&mut source_file, &mut tracing::Span::current().wrap_async_write(expected_size, target_file))
        .await
        .context("copying")?;

    if copied != expected_size {
        anyhow::bail!("[{from:?} -> {to:?}] local copy finished, but received unexpected size (expected [{expected_size}] bytes, downloaded [{copied} bytes])")
    }
    Ok(to)
}
#[instrument]
pub async fn stream_merge_file(from: Vec<url::Url>, to: PathBuf, expected_size: u64) -> Result<PathBuf> {
    let file_name = to
        .file_name()
        .expect("file must have a name")
        .to_string_lossy()
        .to_string();

    let target_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&to)
        .map_with_context(|| format!("opening [{}]", to.display()))
        .await?;

    let mut writer = &mut tracing::Span::current().wrap_async_write(expected_size, target_file);
    let mut downloaded = 0;
    for from_chunk in from.clone().into_iter() {
        let mut byte_stream = reqwest::get(from_chunk.to_string())
            .await
            .with_context(|| format!("making request to {from_chunk}"))?
            .bytes_stream();
        while let Some(chunk) = byte_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    downloaded += chunk.len() as u64;

                    tokio::io::copy(&mut chunk.as_ref(), &mut writer)
                        .await
                        .with_context(|| format!("writing to fd {}", to.display()))?;
                }
                Err(message) => Err(message)?,
            }
        }
    }

    if downloaded != expected_size {
        anyhow::bail!("[{from:?}] download finished, but received unexpected size (expected [{expected_size}] bytes, downloaded [{downloaded} bytes])")
    }
    Ok(to)
}
#[instrument]
pub async fn stream_file(from: url::Url, to: PathBuf, expected_size: u64) -> Result<PathBuf> {
    let file_name = to
        .file_name()
        .expect("file must have a name")
        .to_string_lossy()
        .to_string();

    let target_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&to)
        .map_with_context(|| format!("opening [{}]", to.display()))
        .await?;
    let mut writer = &mut tracing::Span::current().wrap_async_write(expected_size, target_file);
    let mut byte_stream = reqwest::get(from.to_string())
        .await
        .with_context(|| format!("making request to {from}"))?
        .bytes_stream();
    let mut downloaded = 0;
    while let Some(chunk) = byte_stream.next().await {
        match chunk {
            Ok(chunk) => {
                downloaded += chunk.len() as u64;

                tokio::io::copy(&mut chunk.as_ref(), &mut writer)
                    .await
                    .with_context(|| format!("writing to fd {}", to.display()))?;
            }
            Err(message) => Err(message)?,
        }
    }
    if downloaded != expected_size {
        anyhow::bail!("[{from}] download finished, but received unexpected size (expected [{expected_size}] bytes, downloaded [{downloaded} bytes])")
    }
    Ok(to)
}
impl Synchronizers {
    pub fn new(config: DownloadersConfig, games_config: GamesConfig) -> Result<Self> {
        Ok(Self {
            config: Arc::new(config.clone()),
            cache: Arc::new(download_cache::DownloadCache::new(config.downloads_directory.clone()).context("building download cache")?),
            inner: DownloadersInner::new(config).context("building downloaders")?,
            game_synchronizers: Arc::new(get_game_file_source_synchronizers(games_config).context("building game file source synchronizers")?),
        })
    }

    pub async fn prepare_sync_task(self, Archive { descriptor, state }: Archive) -> Result<SyncTask> {
        match state {
            State::Nexus(NexusState {
                game_name, file_id, mod_id, ..
            }) => {
                self.inner
                    .nexus
                    .clone()
                    .context("nexus not configured")
                    .pipe(ready)
                    .and_then(|nexus| {
                        nexus.download(nexus::DownloadFileRequest {
                            // TODO: validate this
                            game_domain_name: game_name,
                            mod_id,
                            file_id,
                        })
                    })
                    .await
                    .map(|url| DownloadTask {
                        inner: (url, self.cache.download_output_path(descriptor.name.clone())),
                        descriptor,
                    })
                    .map(SyncTask::from)
            }
            State::GoogleDrive(GoogleDriveState { id }) => crate::downloaders::google_drive::GoogleDriveDownloader::download(id, descriptor.size)
                .await
                .map(|url| DownloadTask {
                    inner: (url, self.cache.download_output_path(descriptor.name.clone())),
                    descriptor,
                })
                .map(SyncTask::Download),
            State::GameFileSource(state) => self
                .game_synchronizers
                .get(&state.game)
                .with_context(|| format!("check config, no game source configured for [{}]", state.game))
                .pipe(ready)
                .and_then(|synchronizer| synchronizer.prepare_copy(state))
                .await
                .map(|source_path| CopyFileTask {
                    inner: (source_path, self.cache.download_output_path(descriptor.name.clone())),
                    descriptor,
                })
                .map(SyncTask::Copy),

            State::Http(HttpState { url, headers: _ }) => url
                .pipe(|url| DownloadTask {
                    inner: (url, self.cache.download_output_path(descriptor.name.clone())),
                    descriptor,
                })
                .pipe(SyncTask::Download)
                .pipe(Ok),
            State::Manual(ManualState { prompt, url }) => Err(anyhow::anyhow!("Manual action is required:\n\nURL: {url}\n{prompt}")),
            State::WabbajackCDN(state) => WabbajackCDNDownloader::prepare_download(state)
                .await
                .context("wabbajack... :)")
                .map(|source_urls| MergeDownloadTask {
                    inner: (source_urls, self.cache.download_output_path(descriptor.name.clone())),
                    descriptor,
                })
                .map(SyncTask::MergeDownload),
        }
    }

    pub async fn sync_downloads(self, archives: Vec<Archive>) -> TotalResult<WithArchiveDescriptor<PathBuf>> {
        let base_concurrency = 5;
        futures::stream::iter(archives)
            .map(|Archive { descriptor, state }| async {
                match self
                    .cache
                    .clone()
                    .verify(descriptor.clone())
                    .pipe(tokio::task::spawn)
                    .map_context("task crashed")
                    .and_then(ready)
                    .await
                {
                    Ok(verified) => Ok(Either::Left(verified.tap(|verified| tracing::info!(?verified, "succesfully verified a file")))),
                    Err(message) => self
                        .clone()
                        .prepare_sync_task(Archive {
                            descriptor: descriptor.tap(|descriptor| warn!(?descriptor, ?message, "could not verify a file, it will be downloaded")),
                            state,
                        })
                        .await
                        .map(Either::Right),
                }
            })
            .buffer_unordered(base_concurrency)
            .collect::<Vec<_>>()
            .await
            .pipe(futures::stream::iter)
            .map_ok(|file| {
                let name = match &file {
                    Either::Left(left) => left.descriptor.name.clone(),
                    Either::Right(right) => match right {
                        SyncTask::MergeDownload(d) => d.descriptor.name.clone(),
                        SyncTask::Download(d) => d.descriptor.name.clone(),
                        SyncTask::Copy(d) => d.descriptor.name.clone(),
                    },
                };

                match file {
                    Either::Left(exists) => exists.pipe(Ok).pipe(ready).boxed(),
                    Either::Right(sync_task) => match sync_task {
                        SyncTask::MergeDownload(WithArchiveDescriptor { inner: (from, to), descriptor }) => {
                            stream_merge_file(from.clone(), to.clone(), descriptor.size)
                                .map_ok(|inner| WithArchiveDescriptor { inner, descriptor })
                                .map(move |res| res.with_context(|| format!("when downloading [{from:?} -> {to:?}]")))
                                .boxed()
                        }
                        SyncTask::Download(WithArchiveDescriptor { inner: (from, to), descriptor }) => stream_file(from.clone(), to.clone(), descriptor.size)
                            .map_ok(|inner| WithArchiveDescriptor { inner, descriptor })
                            .map(move |res| res.with_context(|| format!("when downloading [{from} -> {to:?}]")))
                            .boxed(),
                        SyncTask::Copy(WithArchiveDescriptor { inner: (from, to), descriptor }) => copy_local_file(from.clone(), to.clone(), descriptor.size)
                            .map_ok(|inner| WithArchiveDescriptor { inner, descriptor })
                            .map(move |res| res.with_context(|| format!("when when copying [{from:?} -> {to:?}]")))
                            .boxed(),
                    },
                }
                .inspect_err({
                    let name = name.clone();
                    move |message| tracing::warn!(?name, ?message)
                })
                .inspect_ok(move |_| tracing::info!(name, "[OK]"))
                .pipe(tokio::task::spawn)
                .map_context("task crashed")
                .and_then(ready)
                .boxed()
            })
            .try_buffer_unordered(base_concurrency * 2)
            .multi_error_collect()
            .await
    }
}

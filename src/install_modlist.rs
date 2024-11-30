use std::{future::ready, path::PathBuf};

use anyhow::{Context, Result};
use downloads::Downloaders;
use futures::TryFutureExt;
use tracing::info;

use crate::{
    config_file::{HoolamikeConfig, InstallationConfig},
    helpers::human_readable_size,
    modlist_json::Modlist,
};
use tap::prelude::*;

pub mod downloads {
    use std::sync::Arc;

    use futures::{FutureExt, StreamExt, TryStreamExt};
    use tokio::sync::RwLock;
    use tracing::debug;

    use super::*;
    use crate::{
        config_file::DownloadersConfig,
        downloaders::nexus::{self, NexusDownloader},
        modlist_json::{Archive, DownloadKind, NexusState, State, UnknownState},
    };

    #[derive(Default, Clone)]
    pub struct DownloadersInner {
        pub nexus: Option<Arc<NexusDownloader>>,
    }

    impl DownloadersInner {
        pub fn new(DownloadersConfig { nexus }: DownloadersConfig) -> Result<Self> {
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
    pub struct Downloaders {
        config: Arc<DownloadersConfig>,
        inner: DownloadersInner,
    }

    impl Downloaders {
        pub fn new(config: DownloadersConfig) -> Self {
            Self {
                config: Arc::new(config),
                inner: Default::default(),
            }
        }

        pub async fn download_archive(
            self,
            Archive {
                hash,
                meta,
                name,
                size,
                state,
            }: Archive,
        ) -> Result<()> {
            // debug!(
            //     ?game,
            //     ?version,
            //     ?id,
            //     ?kind,
            //     ?image_url,
            //     ?url,
            //     ?author,
            //     ?mod_id,
            //     ?name,
            //     ?state_name,
            //     ?size,
            //     "downloading archive"
            // );
            match state {
                State::Nexus(NexusState {
                    game_name,
                    file_id,
                    mod_id,
                    ..
                }) => {
                    self.inner
                        .nexus
                        .clone()
                        .context("nexus not configured")
                        .pipe(ready)
                        .and_then(|nexus| {
                            nexus.download(nexus::DownloadFileRequest {
                                game_domain_name: game_name,
                                mod_id,
                                file_id,
                            })
                        })
                        .await
                }
                State::GameFileSource(kind) => {
                    tracing::error!("{kind:?} is not implemented");
                    Ok(())
                }
                State::GoogleDrive(kind) => {
                    tracing::error!("{kind:?} is not implemented");
                    Ok(())
                }
                State::Http(kind) => {
                    tracing::error!("{kind:?} is not implemented");
                    Ok(())
                }
                State::Manual(kind) => {
                    tracing::error!("{kind:?} is not implemented");
                    Ok(())
                }
                State::WabbajackCDN(kind) => {
                    tracing::error!("{kind:?} is not implemented");
                    Ok(())
                }
            }
        }
        pub async fn download_archives(self, archives: Vec<Archive>) -> Result<()> {
            futures::stream::iter(archives)
                .map(move |archive| self.clone().download_archive(archive))
                .buffer_unordered(100)
                .try_collect::<()>()
                .await
        }
    }
}

#[allow(clippy::needless_as_bytes)]
pub async fn install_modlist(
    HoolamikeConfig {
        downloaders,
        installation:
            InstallationConfig {
                modlist_file,
                original_game_dir,
                installation_dir,
            },
    }: HoolamikeConfig,
) -> Result<()> {
    let downloaders = Downloaders::new(downloaders);

    modlist_file
        .context("no modlist file")
        .and_then(|modlist| {
            std::fs::read_to_string(&modlist)
                .with_context(|| format!("reading modlist at {}", modlist.display()))
                .tap_ok(|read| {
                    info!(
                        "modlist file {} read ({})",
                        modlist.display(),
                        human_readable_size(read.as_bytes().len() as u64)
                    )
                })
        })
        .and_then(|modlist| serde_json::from_str::<Modlist>(&modlist).context("parsing modlist"))
        .pipe(ready)
        .and_then(
            move |Modlist {
                      archives,
                      author,
                      description,
                      directives,
                      game_type,
                      image,
                      is_nsfw,
                      name,
                      readme,
                      version,
                      wabbajack_version,
                      website,
                  }| { downloaders.download_archives(archives) },
        )
        .await
}

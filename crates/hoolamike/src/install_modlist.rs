use {
    crate::{
        config_file::{HoolamikeConfig, InstallationConfig},
        downloaders::WithArchiveDescriptor,
        error::{MultiErrorCollectExt, TotalResult},
        modlist_json::{Archive, Modlist},
        progress_bars::VALIDATE_TOTAL_PROGRESS_BAR,
        wabbajack_file::WabbajackFile,
        DebugHelpers,
    },
    anyhow::Context,
    directives::{DirectivesHandler, DirectivesHandlerConfig},
    downloads::Synchronizers,
    futures::{FutureExt, TryFutureExt, TryStreamExt},
    itertools::Itertools,
    std::{convert::identity, future::ready, sync::Arc},
    tap::prelude::*,
    tracing::info,
};

pub mod download_cache;

pub mod downloads;

pub mod directives;

#[allow(clippy::needless_as_bytes)]
pub async fn install_modlist(
    HoolamikeConfig {
        downloaders,
        installation:
            InstallationConfig {
                whitelist_failed_directives,
                wabbajack_file_path: modlist_file,
                installation_path,
            },
        games,
    }: HoolamikeConfig,
    DebugHelpers {
        skip_verify_and_downloads,
        start_from_directive,
        skip_kind,
        contains,
    }: DebugHelpers,
) -> TotalResult<()> {
    let synchronizers = Synchronizers::new(downloaders.clone(), games.clone())
        .context("setting up downloaders")
        .map_err(|e| vec![e])?;

    let (
        wabbajack_file_handle,
        WabbajackFile {
            wabbajack_file_path: _,
            wabbajack_entries: _,
            modlist,
        },
    ) = tokio::task::spawn_blocking(move || WabbajackFile::load(modlist_file))
        .await
        .context("thread crashed")
        .and_then(identity)
        .context("loading modlist file")
        .tap_ok(|(_, wabbajack)| {
            // PROGRESS
            wabbajack
                .modlist
                .archives
                .iter()
                .map(|archive| archive.descriptor.size)
                .sum::<u64>()
                .pipe(|total_size| {
                    VALIDATE_TOTAL_PROGRESS_BAR.set_length(total_size);
                })
        })
        .map_err(|e| vec![e])?;

    modlist
        .pipe(Ok)
        .pipe(ready)
        .and_then(
            move |Modlist {
                      archives,
                      author: _,
                      description: _,
                      directives,
                      game_type,
                      image: _,
                      is_nsfw: _,
                      name: _,
                      readme: _,
                      version: _,
                      wabbajack_version: _,
                      website: _,
                  }| {
                match skip_verify_and_downloads {
                    true => archives
                        .into_iter()
                        .map(|Archive { descriptor, state: _ }| WithArchiveDescriptor {
                            inner: synchronizers
                                .cache
                                .download_output_path(descriptor.name.clone()),
                            descriptor,
                        })
                        .collect_vec()
                        .pipe(Ok)
                        .pipe(ready)
                        .boxed_local(),
                    false => synchronizers.clone().sync_downloads(archives).boxed_local(),
                }
                .and_then({
                    move |summary| {
                        games
                            .get(&game_type)
                            .with_context(|| format!("[{game_type}] not found in {:?}", games.keys().collect::<Vec<_>>()))
                            .map(|game_config| {
                                DirectivesHandler::new(
                                    DirectivesHandlerConfig {
                                        wabbajack_file: wabbajack_file_handle,
                                        output_directory: installation_path,
                                        failed_directives_whitelist: whitelist_failed_directives,
                                        game_directory: game_config.root_directory.clone(),
                                        downloads_directory: downloaders.downloads_directory.clone(),
                                    },
                                    summary,
                                )
                            })
                            .map_err(|e| vec![e])
                            .pipe(ready)
                    }
                })
                .map_ok(Arc::new)
                .and_then(move |directives_handler| {
                    directives_handler
                        .handle_directives(directives.tap_mut(|directives| {
                            *directives = directives
                                .pipe(std::mem::take)
                                .drain(..)
                                .skip_while(|d| {
                                    start_from_directive
                                        .as_ref()
                                        .map(|start_from_directive| &d.directive_hash() != start_from_directive)
                                        .unwrap_or(false)
                                })
                                .filter(|directive| !skip_kind.contains(&directive.directive_kind()))
                                .filter(|directive| {
                                    serde_json::to_string(&directive)
                                        .tap_err(|e| tracing::error!("{e:#?}"))
                                        .map(|directive| contains.iter().all(|contains| directive.contains(contains)))
                                        .unwrap_or(false)
                                })
                                .collect_vec();
                        }))
                        .multi_error_collect()
                })
            },
        )
        .await
}

use {
    crate::{
        config_file::{HoolamikeConfig, InstallationConfig},
        downloaders::WithArchiveDescriptor,
        error::TotalResult,
        modlist_json::{Archive, Modlist},
        progress_bars::VALIDATE_TOTAL_PROGRESS_BAR,
        wabbajack_file::WabbajackFile,
    },
    anyhow::Context,
    directives::DirectivesHandler,
    downloads::Synchronizers,
    futures::{FutureExt, TryFutureExt},
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
        installation: InstallationConfig {
            wabbajack_file_path: modlist_file,
        },
        games,
    }: HoolamikeConfig,
    skip_verify_and_downloads: bool,
) -> TotalResult<()> {
    let synchronizers = Synchronizers::new(downloaders, games)
        .context("setting up downloaders")
        .map_err(|e| vec![e])?;

    let WabbajackFile {
        wabbajack_file_path,
        wabbajack_entries,
        modlist,
    } = tokio::task::spawn_blocking(move || WabbajackFile::load(modlist_file))
        .await
        .context("thread crashed")
        .and_then(identity)
        .context("loading modlist file")
        .tap_ok(|wabbajack| {
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
                      game_type: _,
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
                .map_ok(DirectivesHandler::new)
                .map_ok(Arc::new)
                .and_then(|directives_handler| directives_handler.handle_directives(directives))
            },
        )
        .await
}

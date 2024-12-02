use {
    crate::{
        config_file::{HoolamikeConfig, InstallationConfig},
        error::TotalResult,
        helpers::human_readable_size,
        modlist_json::Modlist,
        progress_bars::{print_error, VALIDATE_TOTAL_PROGRESS_BAR},
    },
    anyhow::{Context, Result},
    directives::DirectivesHandler,
    downloads::Synchronizers,
    futures::{FutureExt, TryFutureExt},
    std::{future::ready, path::PathBuf, sync::Arc},
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
        installation: InstallationConfig { modlist_file },
        games,
    }: HoolamikeConfig,
) -> TotalResult<()> {
    let synchronizers = Synchronizers::new(downloaders, games)
        .context("setting up downloaders")
        .map_err(|e| vec![e])?;
    let directives_handler = DirectivesHandler::new().pipe(Arc::new);

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
        .tap_ok(|modlist| {
            // PROGRESS
            modlist
                .archives
                .iter()
                .map(|archive| archive.descriptor.size)
                .sum::<u64>()
                .pipe(|total_size| {
                    VALIDATE_TOTAL_PROGRESS_BAR.set_length(total_size);
                })
        })
        .pipe(ready)
        .map_err(|e| vec![e])
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
                synchronizers
                    .clone()
                    .sync_downloads(archives)
                    .and_then(|_sync_summary| directives_handler.handle_directives(directives))
            },
        )
        .await
}

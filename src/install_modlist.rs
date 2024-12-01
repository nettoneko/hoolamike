use {
    crate::{
        config_file::{HoolamikeConfig, InstallationConfig},
        helpers::human_readable_size,
        modlist_json::Modlist,
        progress_bars::{print_error, VALIDATE_TOTAL_PROGRESS_BAR},
    },
    anyhow::{Context, Result},
    downloads::Synchronizers,
    futures::{FutureExt, TryFutureExt},
    std::{future::ready, path::PathBuf},
    tap::prelude::*,
    tracing::info,
};

pub mod download_cache;

pub mod downloads;

#[allow(clippy::needless_as_bytes)]
pub async fn install_modlist(
    HoolamikeConfig {
        downloaders,
        installation: InstallationConfig { modlist_file },
        games,
    }: HoolamikeConfig,
) -> Result<()> {
    let downloaders = Synchronizers::new(downloaders, games).context("setting up downloaders")?;

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
        .and_then(
            move |Modlist {
                      archives,
                      author: _,
                      description: _,
                      directives: _,
                      game_type: _,
                      image: _,
                      is_nsfw: _,
                      name: _,
                      readme: _,
                      version: _,
                      wabbajack_version: _,
                      website: _,
                  }| {
                downloaders
                    .sync_downloads(archives)
                    .map(|errors| match errors.as_slice() {
                        &[] => Ok(()),
                        many_errors => {
                            many_errors.iter().for_each(|error| {
                                print_error("ARCHIVE", error);
                            });
                            print_error("ARCHIVES", &anyhow::anyhow!("could not continue due to [{}] errors", many_errors.len()));
                            Err(errors.into_iter().next().unwrap())
                        }
                    })
            },
        )
        .await
}

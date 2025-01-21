use {
    anyhow::{Context, Result},
    itertools::Itertools,
    std::path::PathBuf,
    tracing::info,
};

#[derive(clap::Args)]
pub struct AudioCliCommand {
    #[command(subcommand)]
    pub command: hoola_audio::Commands,
}

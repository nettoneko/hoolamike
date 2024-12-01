#![allow(clippy::unit_arg)]

use {
    anyhow::{Context, Result},
    clap::{Parser, Subcommand},
    modlist_data::ModlistSummary,
    std::path::PathBuf,
    tap::prelude::*,
    tracing::{info, warn},
};
pub const BUFFER_SIZE: usize = 1024 * 128;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// tests the modlist parser
    ValidateModlist {
        /// path to modlist (json) file
        path: PathBuf,
    },
    /// prints information about the modlist
    ModlistInfo {
        /// path to modlist (json) file
        path: PathBuf,
    },
    Install,
    /// prints prints default config. save it and modify to your liking
    PrintDefaultConfig,
}

pub mod config_file;
pub mod downloaders;
pub mod helpers;
pub mod install_modlist;
pub mod modlist_data;
pub mod modlist_json;
pub(crate) mod progress_bars;

fn setup_logging() {
    use tracing_subscriber::{fmt, prelude::__tracing_subscriber_SubscriberExt, EnvFilter};

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::Layer::new().with_writer(std::io::stderr));
    tracing::subscriber::set_global_default(subscriber)
        .context("Unable to set a global subscriber")
        .expect("logging failed");
}
#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    let Cli { command } = Cli::parse();
    let config = config_file::HoolamikeConfig::find()
        .tap_err(|message| warn!("no config detected, using default config\n{message:#?}"))
        .unwrap_or_default();

    match command {
        Commands::ValidateModlist { path } => tokio::fs::read_to_string(&path)
            .await
            .context("reading test file")
            .and_then(|input| modlist_json::parsing_helpers::validate_modlist_file(&input))
            .with_context(|| format!("testing file {}", path.display())),
        Commands::ModlistInfo { path } => tokio::fs::read_to_string(&path)
            .await
            .context("reading modlist")
            .and_then(|m| serde_json::from_str::<modlist_json::Modlist>(&m).context("parsing modlist"))
            .map(|modlist| ModlistSummary::new(&modlist))
            .map(|modlist| modlist.print())
            .map(|modlist| info!("\n{modlist}")),
        Commands::PrintDefaultConfig => config_file::HoolamikeConfig::default()
            .write()
            .map(|config| println!("{config}")),
        Commands::Install => install_modlist::install_modlist(config).await,
    }
    .with_context(|| {
        format!(
            "\n\nerror occurred, run with --help, check your configuration or file a ticket at {}",
            env!("CARGO_PKG_REPOSITORY")
        )
    })
}

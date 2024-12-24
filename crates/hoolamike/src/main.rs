#![allow(clippy::unit_arg)]
#![feature(seek_stream_len)]
#![feature(slice_take)]
use {
    anyhow::{Context, Result},
    clap::{Args, Parser, Subcommand},
    modlist_data::ModlistSummary,
    modlist_json::DirectiveKind,
    std::path::PathBuf,
    tap::Pipe,
};
pub const BUFFER_SIZE: usize = 1024 * 64;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// the hoolamike config file is where you configure your installation - we're linux users, we can't afford windows
    /// which means we can't afford GUI-capable hardware anyway
    ///
    /// in the config you'll have to specify a modlist file - you'll have to download it
    /// can it be downloaded autside of wabbajack gui client?
    /// yes and no
    /// they can be found here: https://build.wabbajack.org/authored_files **BUT** the manual download should be avoided unless absolutely necessary.
    /// probably best approach would be visiting official Wabbajack discord server and asking someone which file is safe to download
    #[arg(long, short = 'c', default_value = std::env::current_dir().unwrap().join("hoolamike.yaml").into_os_string())]
    hoolamike_config: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Args, Default)]
pub struct DebugHelpers {
    /// skip verification (used mostly for developing the tool)
    #[arg(long)]
    skip_verify_and_downloads: bool,
    #[arg(long)]
    start_from_directive: Option<String>,
    #[arg(long)]
    skip_kind: Vec<DirectiveKind>,
    #[arg(long)]
    contains: Vec<String>,
}

#[derive(Subcommand)]
enum HoolamikeDebugCommand {
    ReserializeDirectives { modlist_file: PathBuf },
}

#[derive(Args)]
struct HoolamikeDebug {
    #[command(subcommand)]
    command: HoolamikeDebugCommand,
}

#[derive(Subcommand)]
enum Commands {
    HoolamikeDebug(HoolamikeDebug),
    /// tests the modlist parser
    ValidateModlist {
        /// path to modlist (.wabbajack) file
        path: PathBuf,
    },
    /// prints information about the modlist
    ModlistInfo {
        /// path to modlist (.wabbajack) file
        path: PathBuf,
    },
    Install {
        #[command(flatten)]
        debug: DebugHelpers,
    },
    /// prints prints default config. save it and modify to your liking
    PrintDefaultConfig,
}

pub mod read_wrappers;
#[macro_use]
pub mod utils;

pub mod compression;
pub mod config_file;
pub mod downloaders;
pub mod error;
pub mod helpers;
pub mod install_modlist;
pub mod modlist_data;
pub mod modlist_json;
pub mod octadiff_reader;
pub mod progress_bars_v2;
pub mod wabbajack_file {
    use {
        crate::{
            compression::ProcessArchive,
            install_modlist::directives::{WabbajackFileHandle, WabbajackFileHandleExt},
            progress_bars_v2::IndicatifWrapIoExt,
        },
        anyhow::{Context, Result},
        std::path::{Path, PathBuf},
    };

    #[derive(Debug)]
    pub struct WabbajackFile {
        pub wabbajack_file_path: PathBuf,
        pub wabbajack_entries: Vec<PathBuf>,
        pub modlist: super::modlist_json::Modlist,
    }

    const MODLIST_JSON_FILENAME: &str = "modlist";

    impl WabbajackFile {
        #[tracing::instrument]
        pub fn load(path: PathBuf) -> Result<(WabbajackFileHandle, Self)> {
            crate::compression::wrapped_7zip::WRAPPED_7ZIP
                .with(|w| w.open_file(&path))
                .context("reading archive")
                .and_then(|mut archive| {
                    archive.list_paths().and_then(|entries| {
                        archive
                            .get_handle(Path::new(MODLIST_JSON_FILENAME))
                            .context("looking up file by name")
                            .and_then(|handle| {
                                serde_json::from_reader::<_, crate::modlist_json::Modlist>(&mut tracing::Span::current().wrap_read(0, handle))
                                    .context("reading archive contents")
                            })
                            .with_context(|| format!("reading [{MODLIST_JSON_FILENAME}]"))
                            .map(|modlist| Self {
                                wabbajack_file_path: path,
                                wabbajack_entries: entries,
                                modlist,
                            })
                            .map(|data| (WabbajackFileHandle::from_archive(archive), data))
                    })
                })
        }
    }
}

#[allow(unused_imports)]
fn setup_logging() {
    use {
        tracing_indicatif::IndicatifLayer,
        tracing_subscriber::{fmt, layer::SubscriberExt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter},
    };
    let indicatif_layer = IndicatifLayer::new();
    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer);
    // .pipe(|registry| {
    //     // #[cfg(debug_assertions)]
    //     {
    //         // registry.with(console_subscriber::spawn())
    //     }
    //     // #[cfg(not(debug_assertions))]
    //     // {
    //     // registry.with(fmt::Layer::new().with_writer(std::io::stderr))
    //     // }
    //     // registry
    // });
    tracing::subscriber::set_global_default(subscriber)
        .context("Unable to set a global subscriber")
        .expect("logging failed");
}
#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    let Cli { command, hoolamike_config } = Cli::parse();

    match command {
        Commands::ValidateModlist { path } => tokio::fs::read_to_string(&path)
            .await
            .context("reading test file")
            .and_then(|input| modlist_json::parsing_helpers::validate_modlist_file(&input))
            .with_context(|| format!("testing file {}", path.display())),
        Commands::ModlistInfo { path } => wabbajack_file::WabbajackFile::load(path)
            .context("reading modlist")
            .map(|(_, modlist)| ModlistSummary::new(&modlist.modlist))
            .map(|modlist| modlist.print())
            .map(|modlist| println!("\n{modlist}")),
        Commands::PrintDefaultConfig => config_file::HoolamikeConfig::default()
            .write()
            .map(|config| println!("{config}")),
        Commands::Install { debug } => {
            let (config_path, config) = config_file::HoolamikeConfig::find(&hoolamike_config).context("reading hoolamike config file")?;
            tracing::info!("found config at [{}]", config_path.display());

            install_modlist::install_modlist(config, debug)
                .await
                .map_err(|errors| {
                    errors
                        .iter()
                        .enumerate()
                        .for_each(|(idx, reason)| eprintln!("{idx}. {reason:?}", idx = idx + 1));
                    anyhow::anyhow!("could not finish installation due to [{}] errors", errors.len())
                })
                .map(|count| println!("successfully installed [{}] mods", count.len()))
        }
        Commands::HoolamikeDebug(HoolamikeDebug { command }) => match command {
            HoolamikeDebugCommand::ReserializeDirectives { modlist_file } => wabbajack_file::WabbajackFile::load(modlist_file)
                .context("loading modlist file")
                .and_then(|modlist| {
                    modlist
                        .1
                        .modlist
                        .directives
                        .pipe_ref(|directives| serde_json::to_string_pretty(directives).context("serializing directives"))
                })
                .map(|directives| println!("{directives}")),
        },
    }
    .with_context(|| {
        format!(
            "\n\nerror occurred, run with --help, check your configuration or file a ticket at {}",
            env!("CARGO_PKG_REPOSITORY")
        )
    })
}

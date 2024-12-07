#![allow(clippy::unit_arg)]
#![feature(seek_stream_len)]
use {
    anyhow::{Context, Result},
    clap::{Parser, Subcommand},
    modlist_data::ModlistSummary,
    progress_bars::print_success,
    std::path::PathBuf,
    tap::prelude::*,
    tracing::info,
};
pub const BUFFER_SIZE: usize = 1024 * 128;

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

#[derive(Subcommand)]
enum Commands {
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
        /// skip verification (used mostly for developing the tool)
        #[arg(long)]
        skip_verify_and_downloads: bool,
    },
    /// prints prints default config. save it and modify to your liking
    PrintDefaultConfig,
}

pub mod utils {
    use {
        itertools::Itertools,
        serde::{Deserialize, Serialize},
        std::path::PathBuf,
    };

    #[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd, Hash, derive_more::Display)]
    pub struct MaybeWindowsPath(pub String);

    impl MaybeWindowsPath {
        pub fn into_path(self) -> PathBuf {
            let s = self.0;
            let s = match s.contains("\\\\") {
                true => s.split("\\\\").join("/"),
                false => s,
            };
            let s = match s.contains("\\") {
                true => s.split("\\").join("/"),
                false => s,
            };
            PathBuf::from(s)
        }
    }

    pub fn boxed_iter<'a, T: 'a>(iter: impl Iterator<Item = T> + 'a) -> Box<dyn Iterator<Item = T> + 'a> {
        Box::new(iter)
    }
}

pub mod error;

pub mod compression;
pub mod config_file;
pub mod downloaders;
pub mod helpers;
pub mod install_modlist;
pub mod modlist_data;
pub mod modlist_json;
pub mod octadiff_reader;
pub mod wabbajack_file {
    use {
        crate::{
            compression::ProcessArchive,
            install_modlist::directives::{WabbajackFileHandle, WabbajackFileHandleExt},
        },
        anyhow::{Context, Result},
        std::path::{Path, PathBuf},
        tap::prelude::*,
    };

    #[derive(Debug)]
    pub struct WabbajackFile {
        pub wabbajack_file_path: PathBuf,
        pub wabbajack_entries: Vec<PathBuf>,
        pub modlist: super::modlist_json::Modlist,
    }

    const MODLIST_JSON_FILENAME: &str = "modlist";

    impl WabbajackFile {
        pub fn load(path: PathBuf) -> Result<(WabbajackFileHandle, Self)> {
            let pb = indicatif::ProgressBar::new_spinner()
                .with_prefix(path.display().to_string())
                .tap_mut(|pb| crate::progress_bars::ProgressKind::Validate.stylize(pb));
            std::fs::OpenOptions::new()
                .read(true)
                .open(&path)
                .context("opening file")
                .and_then(|file| crate::compression::zip::ZipArchive::new(file).context("reading archive"))
                .and_then(|mut archive| {
                    archive.list_paths().and_then(|entries| {
                        archive
                            .get_handle(Path::new(MODLIST_JSON_FILENAME))
                            .context("looking up file by name")
                            .and_then(|handle| {
                                serde_json::from_reader::<_, crate::modlist_json::Modlist>(&mut pb.wrap_read(handle)).context("reading archive contents")
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
pub(crate) mod progress_bars;

#[allow(unused_imports)]
fn setup_logging() {
    use tracing_subscriber::{fmt, prelude::__tracing_subscriber_SubscriberExt, EnvFilter};

    let subscriber = tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .pipe(|registry| {
            // #[cfg(debug_assertions)]
            {
                // registry.with(console_subscriber::spawn())
            }
            // #[cfg(not(debug_assertions))]
            // {
            //     registry.with(fmt::Layer::new().with_writer(std::io::stderr))
            // }
            registry
        });
    tracing::subscriber::set_global_default(subscriber)
        .context("Unable to set a global subscriber")
        .expect("logging failed");
}
#[tokio::main]
async fn main() -> Result<()> {
    setup_logging();
    let Cli { command, hoolamike_config } = Cli::parse();
    let (config_path, config) = config_file::HoolamikeConfig::find(&hoolamike_config).context("reading hoolamike config file")?;
    print_success("hoolamike".into(), &format!("found config at [{}]", config_path.display()));

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
        Commands::Install { skip_verify_and_downloads } => install_modlist::install_modlist(config, skip_verify_and_downloads)
            .await
            .map_err(|errors| {
                errors
                    .iter()
                    .enumerate()
                    .for_each(|(idx, reason)| eprintln!("{idx}. {reason:?}", idx = idx + 1));
                anyhow::anyhow!("could not finish installation due to [{}] errors", errors.len())
            })
            .map(|count| info!("successfully installed [{}] mods", count.len())),
    }
    .with_context(|| {
        format!(
            "\n\nerror occurred, run with --help, check your configuration or file a ticket at {}",
            env!("CARGO_PKG_REPOSITORY")
        )
    })
}

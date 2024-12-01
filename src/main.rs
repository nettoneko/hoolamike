#![allow(clippy::unit_arg)]

use anyhow::{Context, Result};
use indicatif::MultiProgress;
use modlist_data::ModlistSummary;
use std::path::PathBuf;
use tap::prelude::*;
use tracing::{debug, info, warn};

use clap::{Parser, Subcommand};

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

pub mod config_file {
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use serde::{Deserialize, Serialize};
    use tap::prelude::*;
    use tracing::{debug, info, warn};

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct NexusConfig {
        pub api_key: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
    #[derivative(Default)]
    pub struct DownloadersConfig {
        #[derivative(Default(value = "std::env::current_dir().unwrap().join(\"downloads\")"))]
        pub downloads_directory: PathBuf,
        pub nexus: NexusConfig,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct InstallationConfig {
        pub modlist_file: Option<PathBuf>,
        pub original_game_dir: Option<PathBuf>,
        pub installation_dir: Option<PathBuf>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct HoolamikeConfig {
        pub downloaders: DownloadersConfig,
        pub installation: InstallationConfig,
    }

    pub static CONFIG_FILE_NAME: &str = "hoolamike.yaml";
    impl HoolamikeConfig {
        pub fn write(&self) -> Result<String> {
            Self::default()
                .pipe_ref(serde_yaml::to_string)
                .context("serialization failed")
                .map(|config| format!("\n# default {CONFIG_FILE_NAME} file\n# edit it according to your needs:\n{config}"))
        }
        pub fn find() -> Result<Self> {
            [
                format!("./{CONFIG_FILE_NAME}"),
                format!("~/.config/hoolamike/{CONFIG_FILE_NAME}"),
            ]
            .pipe(|config_paths| {
                config_paths
                    .clone()
                    .into_iter()
                    .map(PathBuf::from)
                    .find(|path| path.exists())
                    .with_context(|| format!("checking paths: {config_paths:?}"))
                    .context("no config file detected")
            })
            .tap_ok(|config| info!("found config at '{}'", config.display()))
            .and_then(|config| std::fs::read_to_string(config).context("reading file"))
            .and_then(|config| serde_yaml::from_str::<Self>(&config).context("parsing config file"))
            .with_context(|| format!("getting [{CONFIG_FILE_NAME}]"))
            .tap_ok(|config| {
                debug!("{config:?}");
            })
        }
    }
}

pub mod modlist_json;
pub mod helpers {
    pub fn human_readable_size(bytes: u64) -> String {
        const UNITS: [&str; 6] = ["B", "kB", "MB", "GB", "TB", "PB"];

        if bytes < 1024 {
            return format!("{} {}", bytes, UNITS[0]);
        }

        let exponent = (bytes as f64).log(1024.0).floor() as usize;
        let exponent = exponent.min(UNITS.len() - 1);
        let value = bytes as f64 / 1024f64.powi(exponent as i32);

        format!("{:.2} {}", value, UNITS[exponent])
    }
}
pub mod modlist_data {
    use itertools::Itertools;
    use std::collections::BTreeMap;
    use tabled::{
        settings::{object::Columns, Color, Rotate, Style},
        Tabled,
    };
    use tap::prelude::*;

    use crate::{helpers::human_readable_size, modlist_json::Modlist};

    #[derive(Tabled)]
    pub struct ModlistSummary {
        pub author: String,
        pub total_mods: usize,
        pub total_directives: usize,
        pub unique_directive_kinds: String,
        // pub unique_authors: usize,
        pub sources: String,
        pub name: String,
        // pub unique_headers: String,
        pub website: String,
        pub total_download_size: String,
        pub description: String,
    }

    fn summarize_value_count<'a, I: std::fmt::Display + Ord + Clone + Eq>(
        items: impl Iterator<Item = I> + 'a,
    ) -> String {
        items
            .fold(BTreeMap::new(), |acc, directive| {
                acc.tap_mut(move |acc| {
                    *acc.entry(directive).or_insert(0) += 1;
                })
            })
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .join("\n")
    }
    impl ModlistSummary {
        pub fn print(&self) -> String {
            tabled::Table::new([self])
                .with(Style::modern())
                .with(Rotate::Left)
                .modify(Columns::single(0), Color::FG_GREEN)
                .to_string()
        }

        pub fn new(
            Modlist {
                archives,
                author,
                description,
                directives,
                name,
                website,
                is_nsfw: _,
                game_type: _,
                image: _,
                readme: _,
                version: _,
                wabbajack_version: _,
            }: &Modlist,
        ) -> Self {
            Self {
                author: author.clone(),
                sources: archives
                    .iter()
                    .map(|archive| archive.state.kind().to_string())
                    .pipe(summarize_value_count),
                total_mods: archives.len(),
                // unique_authors: archives
                //     .iter()
                //     .filter_map(|archive| archive.state.author.as_ref())
                //     .unique()
                //     .count(),
                total_directives: directives.len(),
                unique_directive_kinds: directives
                    .iter()
                    .map(|d| d.directive_kind)
                    .pipe(summarize_value_count),
                name: name.clone(),
                // unique_headers: archives
                //     .iter()
                //     .flat_map(|a| {
                //         a.state
                //             .headers
                //             .iter()
                //             .flat_map(|m| m.iter().map(|h| h.as_str()))
                //     })
                //     .unique()
                //     .join(",\n"),
                website: website.clone(),
                total_download_size: archives
                    .iter()
                    .map(|a| a.descriptor.size)
                    .sum::<u64>()
                    .pipe(human_readable_size),
                description: description.clone(),
            }
        }
    }
}

pub mod downloaders;

pub mod install_modlist;

pub(crate) mod progress_bars {
    use console::style;
    use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
    use once_cell::sync::Lazy;
    use tap::prelude::*;

    pub(crate) static PROGRESS_BAR: Lazy<MultiProgress> = Lazy::new(MultiProgress::new);
    pub fn vertical_progress_bar(len: u64, color: &str) -> ProgressBar {
        ProgressBar::new(len).tap_mut(|pb| {
            pb.set_style(
                ProgressStyle::with_template(
                    &format!("{{prefix:.bold}}▕{{bar:.{color}}}▏{{msg:.{color}}} ({{bytes}}/{{total_bytes}}, ETA {{eta}})"),
                )
                .unwrap()
                .progress_chars("█▇▆▅▄▃▂▁  "),
            );
        })
    }

    pub fn print_error(for_target: &str, message: &anyhow::Error) {
        PROGRESS_BAR
            .println(format!(
                "{} {message}",
                style(for_target).bold().dim().red(),
            ))
            .ok();
    }
    pub fn print_warn(for_target: &str, message: &anyhow::Error) {
        PROGRESS_BAR
            .println(format!(
                "{} {message}",
                style(for_target).bold().dim().yellow(),
            ))
            .ok();
    }
    pub fn print_success(for_target: &str, message: &str) {
        PROGRESS_BAR
            .println(format!(
                "{} {message}",
                style(for_target).bold().dim().green(),
            ))
            .ok();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
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
            .and_then(|m| {
                serde_json::from_str::<modlist_json::Modlist>(&m).context("parsing modlist")
            })
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
            "error occurred, run with --help, check your configuration or file a ticket at {}",
            env!("CARGO_PKG_REPOSITORY")
        )
    })
}

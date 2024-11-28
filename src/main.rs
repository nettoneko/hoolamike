use anyhow::{Context, Result};
use modlist_data::ModlistSummary;
use std::path::PathBuf;
use tap::prelude::*;
use tracing::info;

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
    ValidateModlist { path: PathBuf },
    /// prints information about the modlist
    ModlistInfo { path: PathBuf },
}

pub mod modlist_json;
pub mod modlist_data {
    use std::collections::BTreeMap;

    use anyhow::Context;
    use clap::Subcommand;
    use itertools::Itertools;
    use tabled::{
        settings::{object::Columns, panel::Header, Color, Rotate, Style},
        Tabled,
    };
    use tap::prelude::*;

    use crate::modlist_json::Modlist;

    #[derive(Tabled)]
    pub struct ModlistSummary {
        pub author: String,
        pub total_mods: usize,
        pub total_directives: usize,
        pub unique_directive_kinds: String,
        pub unique_authors: usize,
        pub sources: String,
        pub name: String,
        pub unique_headers: String,
        pub website: String,
        pub total_download_size: String,
        pub description: String,
    }
    fn human_readable_size(bytes: u64) -> String {
        const UNITS: [&str; 6] = ["B", "kB", "MB", "GB", "TB", "PB"];

        if bytes < 1024 {
            return format!("{} {}", bytes, UNITS[0]);
        }

        let exponent = (bytes as f64).log(1024.0).floor() as usize;
        let exponent = exponent.min(UNITS.len() - 1);
        let value = bytes as f64 / 1024f64.powi(exponent as i32);

        format!("{:.2} {}", value, UNITS[exponent])
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
                game_type,
                image,
                is_nsfw,
                name,
                readme,
                version,
                wabbajack_version,
                website,
            }: &Modlist,
        ) -> Self {
            Self {
                author: author.clone(),
                sources: archives
                    .iter()
                    .map(|archive| archive.state.kind.to_string())
                    .pipe(summarize_value_count),
                total_mods: archives.len(),
                unique_authors: archives
                    .iter()
                    .filter_map(|archive| archive.state.author.as_ref())
                    .unique()
                    .count(),
                total_directives: directives.len(),
                unique_directive_kinds: directives
                    .iter()
                    .map(|d| d.directive_kind)
                    .pipe(summarize_value_count),
                name: name.clone(),
                unique_headers: archives
                    .iter()
                    .flat_map(|a| {
                        a.state
                            .headers
                            .iter()
                            .flat_map(|m| m.iter().map(|h| h.as_str()))
                    })
                    .unique()
                    .join(",\n"),
                website: website.clone(),
                total_download_size: archives
                    .iter()
                    .map(|a| a.size)
                    .sum::<u64>()
                    .pipe(human_readable_size),
                description: description.clone(),
            }
        }
    }
}

pub mod downloaders {
    pub mod gamefile_source_downloader {
        pub struct GameFileSourceDownloader {}
    }
    pub mod google_drive {
        pub struct GoogleDriveDownloader {}
    }
    pub mod http {
        pub struct HttpDownloader {}
    }
    pub mod manual {
        pub struct ManualDownloader {}
    }
    pub mod nexus {
        pub struct NexusDownloader {}
    }
    pub mod wabbajack_cdn {
        pub struct WabbajackCDNDownloader {}
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let Cli { command } = Cli::parse();
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
    }
}

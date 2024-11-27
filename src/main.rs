use anyhow::{Context, Result};
use std::path::PathBuf;
use tap::prelude::*;

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
    TestModlist { path: PathBuf },
}

pub mod modlist_json;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let Cli { command } = Cli::parse();
    match command {
        Commands::TestModlist { path } => tokio::fs::read_to_string(&path)
            .await
            .context("reading test file")
            .and_then(|input| modlist_json::parsing_helpers::test_modlist_file(&input))
            .with_context(|| format!("testing file {}", path.display())),
    }
}

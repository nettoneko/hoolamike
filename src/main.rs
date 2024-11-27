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

pub mod modlist_json {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct Modlist {
        pub archives: Vec<Archive>,
        pub author: String,
        pub description: String,
        pub directives: Vec<Directive>,
        pub game_type: String,
        pub image: String,
        #[serde(rename = "IsNSFW")]
        pub is_nsfw: bool,
        pub name: String,
        pub readme: String,
        pub version: String,
        pub wabbajack_version: String,
        pub website: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct Archive {
        pub hash: String,
        pub meta: String,
        pub name: String,
        pub size: u64,
        pub state: State,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct State {
        pub id: Option<String>,
        pub name: Option<String>,
        pub prompt: Option<String>,
        pub version: Option<String>,
        #[serde(rename = "$type")]
        pub kind: String,
        pub game: Option<String>,
        pub game_file: Option<String>,
        pub game_version: Option<String>,
        pub game_name: Option<String>,
        #[serde(rename = "ImageURL")]
        pub image_url: Option<String>,
        pub hash: Option<String>,
        pub headers: Option<Vec<String>>, // Assuming headers is a list of strings, adjust if necessary
        pub url: Option<String>,
        pub author: Option<String>,
        pub description: Option<String>,
        #[serde(rename = "FileID")]
        pub file_id: Option<usize>,
        #[serde(rename = "IsNSFW")]
        pub is_nsfw: Option<bool>,
        #[serde(rename = "ModID")]
        pub mod_id: Option<usize>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct Directive {
        #[serde(rename = "$type")]
        pub directive_type: String,
        pub hash: String,
        pub size: u64,
        #[serde(rename = "SourceDataID")]
        pub source_data_id: Option<String>,
        pub image_state: Option<ImageState>,
        pub to: String,
        pub archive_hash_path: Option<Vec<String>>,
        pub from_hash: Option<String>,
        #[serde(rename = "PatchID")]
        pub patch_id: Option<String>,
        #[serde(rename = "TempID")]
        pub temp_id: Option<String>,
        pub file_states: Option<Vec<FileState>>,
        pub state: Option<DirectiveState>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct ImageState {
        pub format: String,
        pub height: u64,
        pub mip_levels: u64,
        pub perceptual_hash: String,
        pub width: u64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct DirectiveState {
        #[serde(rename = "$type")]
        pub directive_state_type: String,
        pub has_name_table: bool,
        pub header_magic: String,
        #[serde(rename = "Type")]
        pub kind: u64,
        pub version: u64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct FileState {
        #[serde(rename = "$type")]
        pub file_state_type: String,
        pub align: Option<u64>,
        pub compressed: Option<bool>,
        pub dir_hash: u64,
        pub chunk_hdr_len: Option<u64>,
        pub chunks: Option<Vec<FileStateChunk>>,
        pub num_mips: Option<u64>,
        pub pixel_format: Option<u64>,
        pub tile_mode: Option<u64>,
        #[serde(rename = "Unk8")]
        pub unk_8: Option<u8>,
        pub extension: String,
        pub height: Option<u64>,
        pub width: Option<u64>,
        pub is_cube_map: Option<u8>,
        pub flags: Option<u64>,
        pub index: usize,
        pub name_hash: u64,
        pub path: PathBuf,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    #[serde(rename_all = "PascalCase")]
    pub struct FileStateChunk {
        pub align: u64,
        pub compressed: bool,
        pub end_mip: u64,
        pub full_sz: u64,
        pub start_mip: u64,
    }
}
pub mod parsing_helpers {
    use std::{collections::BTreeMap, iter::repeat, ops::Not};

    use anyhow::{Context, Result};
    use itertools::Itertools;
    use serde_json::Value;
    use tap::prelude::*;
    use tracing::info;

    #[derive(Debug)]
    enum ValueSummary<'a> {
        Map {
            fields: BTreeMap<&'a str, Self>,
        },
        Array {
            first_element: Option<Box<Self>>,
            len: usize,
        },
        Other(&'a serde_json::Value),
    }

    fn summarize_node(node: &Value) -> ValueSummary {
        match node {
            Value::Array(vec) => ValueSummary::Array {
                first_element: vec.first().map(summarize_node).map(Box::new),
                len: vec.len(),
            },
            Value::Object(map) => ValueSummary::Map {
                fields: map
                    .iter()
                    .map(|(key, value)| (key.as_str(), summarize_node(value)))
                    .collect(),
            },
            other => ValueSummary::Other(other),
        }
    }

    mod ad_hoc_test {
        use super::*;

        #[cfg(ignore)]
        #[test_log::test]
        fn test_wasteland_reborn() -> Result<()> {
            include_str!("../../wasteland-reborn/test/modlist").pipe(test_modlist_file)
        }
    }

    pub fn test_modlist_file(input: &str) -> Result<()> {
        input
            .tap(|input| {
                info!("file is {} bytes long", input.bytes().len());
            })
            .pipe_as_ref(serde_json::from_str::<Value>)
            .context("bad json")
            .tap_ok(|node| summarize_node(node).pipe(|summary| info!("{summary:#?}")))
            .and_then(|node| serde_json::to_string_pretty(&node).context("serializing"))
            .and_then(move |pretty_input| {
                serde_json::from_str::<crate::modlist_json::Modlist>(&pretty_input)
                    .pipe(|res| match res.as_ref() {
                        Ok(_) => res.context(""),
                        Err(e) => e.line().pipe(|line| {
                            res.with_context(|| {
                                pretty_input
                                    .lines()
                                    .enumerate()
                                    .map(|(idx, line)| format!("{}. {line}", idx + 1))
                                    .skip(line - 3)
                                    .take(6)
                                    .join("\n")
                            })
                        }),
                    })
                    .context("bad modlist")
            })
            .map(|_| ())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let Cli { command } = Cli::parse();
    match command {
        Commands::TestModlist { path } => tokio::fs::read_to_string(&path)
            .await
            .context("reading test file")
            .and_then(|input| parsing_helpers::test_modlist_file(&input))
            .with_context(|| format!("testing file {}", path.display())),
    }
}

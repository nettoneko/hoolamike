use {
    crate::{modlist_json::HumanUrl, utils::deserialize_json_with_error_location},
    anyhow::{Context, Result},
    serde::{Deserialize, Serialize},
};

pub mod kind_guard;

pub mod location;
pub mod variable;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Tag {
    pub name: String,
    #[serde(rename = "ID")]
    pub id: u16,
    pub text_color: String,
    pub back_color: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Check {
    #[serde(rename = "Type")]
    pub kind: u8,
    pub inverted: bool,
    pub loc: u8,
    pub file: String,
    pub custom_message: String,
    pub checksums: Option<String>,
    pub free_size: Option<u64>,
}

/// this one is super weird but ok
pub mod asset;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct FileAttr {
    pub value: String,
    pub last_modified: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct PostCommand {
    pub value: String,
    pub wait: bool,
    pub hidden: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DebugAndRelease<T>((Vec<T>, Vec<T>));

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Gui {
    pub files: String,
    pub width: u32,
    pub height: u32,
    pub borderless: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Package {
    pub title: String,
    pub version: String,
    pub author: String,
    pub home_page: HumanUrl,
    pub description: String,
    #[serde(rename = "GUI")]
    pub gui: Gui,
}

/// Tale of two Wastelands installer manifest file
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Manifest {
    pub package: Package,
    pub variables: DebugAndRelease<variable::Variable>,
    pub locations: DebugAndRelease<location::Location>,
    pub tags: Vec<Tag>,
    pub checks: Vec<Check>,
    pub file_attrs: Vec<FileAttr>,
    pub post_commands: Vec<PostCommand>,
    pub assets: Vec<asset::Asset>,
}

#[test_log::test]
fn test_ad_hoc_example_manifest_file() -> Result<()> {
    let example = include_str!("../../../../../playground/begin-again/ttw-installer/ttw-mpi-extracted/_package/index.json");
    serde_json::from_str::<serde_json::Value>(example)
        .context("deserializing json")
        .and_then(|v| serde_json::to_string_pretty(&v).context("reserializing raw json"))
        .and_then(|example| deserialize_json_with_error_location::<Manifest>(&example).context("deserializing manifest"))
        .and_then(|manifest| {
            serde_json::to_string(&manifest)
                .context("reserializing")
                .and_then(|reserialized| deserialize_json_with_error_location::<Manifest>(&reserialized).context("deserializing reserialized json"))
                .and_then(|from_reserialized| {
                    manifest
                        .eq(&from_reserialized)
                        .then_some(())
                        .context("reserialization should not be lossy")
                })
        })
}

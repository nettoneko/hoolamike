use {
    crate::modlist_json::HumanUrl,
    anyhow::{Context, Result},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Variable {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: u8,
    pub exclude_delimiter: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Variables((Vec<Variable>, Vec<Variable>));

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Gui {
    pub files: String,
    pub width: u32,
    pub height: u32,
    pub borderless: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Manifest {
    pub package: Package,
}

#[test]
fn test_ad_hoc_example_manifest_file() -> Result<()> {
    let example = include_str!("../../../../../playground/begin-again/ttw-installer/ttw-mpi-extracted/_package/index.json");

    serde_json::from_str::<Manifest>(example)
        .context("bad json")
        .map(|_| ())
}

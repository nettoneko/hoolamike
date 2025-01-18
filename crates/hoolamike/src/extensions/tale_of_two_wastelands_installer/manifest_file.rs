use {
    crate::modlist_json::HumanUrl,
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

pub mod check;

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
pub struct DebugAndRelease<T>(Vec<T>, Vec<T>);

impl<T> DebugAndRelease<T> {
    pub fn release(self) -> Vec<T> {
        self.1
    }
}

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
    pub checks: Vec<check::Check>,
    pub file_attrs: Vec<FileAttr>,
    pub post_commands: Vec<PostCommand>,
    pub assets: Vec<asset::Asset>,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{post_install_fixup::diffing::PrettyDiff, utils::deserialize_json_with_error_location},
        anyhow::Context,
    };

    #[test_log::test]
    fn test_ad_hoc_example_manifest_file() -> anyhow::Result<()> {
        let example = include_str!("../../../../../playground/begin-again/ttw-installer/ttw-mpi-extracted/_package/index.json");
        serde_json::from_str::<serde_json::Value>(example)
            .context("deserializing json")
            .and_then(|v| serde_json::to_string_pretty(&v).context("reserializing raw json"))
            .and_then(|pretty_example| {
                deserialize_json_with_error_location::<Manifest>(&pretty_example)
                    .context("deserializing manifest")
                    .and_then(|manifest| {
                        serde_json::to_string_pretty(&manifest)
                            .context("reserializing")
                            .map(|reserialized| {
                                assert_json_diff::assert_json_eq!(
                                    serde_json::from_str::<serde_json::Value>(&pretty_example).unwrap(),
                                    serde_json::from_str::<serde_json::Value>(&reserialized).unwrap(),
                                )
                            })
                    })
            })
    }
}

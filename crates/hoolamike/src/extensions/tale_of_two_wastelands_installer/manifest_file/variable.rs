use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Variable {
    pub name: String,
    #[serde(rename = "Type")]
    pub kind: u8,
    #[serde(default)]
    pub exclude_delimiter: bool,
    pub value: Option<String>,
}

use serde::{Deserialize, Serialize};

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

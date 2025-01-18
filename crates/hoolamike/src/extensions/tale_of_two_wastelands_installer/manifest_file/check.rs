use {
    super::{
        asset::{FileName, LocationIndex},
        kind_guard::WithKindGuard,
    },
    serde::{Deserialize, Serialize},
};

/// 8 chars hexadecimal for CRC32 or 32 chars hexadecimal for MD5
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Checksums(String);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct FileExistsCheck {
    pub inverted: bool,
    pub loc: LocationIndex,
    pub file: FileName,
    pub custom_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksums: Option<Checksums>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct FreeSizeCheck {
    pub inverted: bool,
    pub loc: u8,
    pub file: String,
    pub custom_message: String,
    pub free_size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct NoProgramFilesCheck {
    pub inverted: bool,
    pub loc: u8,
    pub file: String,
    pub custom_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Check {
    FileExists(WithKindGuard<0, FileExistsCheck>),
    FreeSize(WithKindGuard<1, FreeSizeCheck>),
    NoProgramFiles(WithKindGuard<2, NoProgramFilesCheck>),
}

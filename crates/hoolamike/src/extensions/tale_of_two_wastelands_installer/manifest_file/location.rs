use {
    super::kind_guard::WithKindGuard,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct FolderLocation {
    pub name: String,
    pub value: String,
    pub create_folder: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct ReadArchiveLocation {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct WriteArchiveLocation {
    pub name: String,
    pub value: String,
    pub archive_type: u16,
    pub archive_flags: u16,
    pub files_flags: u16,
    pub archive_compressed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Location {
    Folder(WithKindGuard<0, FolderLocation>),
    ReadArchive(WithKindGuard<1, ReadArchiveLocation>),
    WriteArchive(WithKindGuard<2, WriteArchiveLocation>),
}

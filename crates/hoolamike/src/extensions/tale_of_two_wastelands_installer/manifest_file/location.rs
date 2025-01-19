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

impl Location {
    pub fn name(&self) -> &str {
        match self {
            Location::Folder(l) => l.inner.name.as_str(),
            Location::ReadArchive(l) => l.inner.name.as_str(),
            Location::WriteArchive(l) => l.inner.name.as_str(),
        }
    }
    pub fn value_mut(&mut self) -> &mut String {
        match self {
            Location::Folder(WithKindGuard {
                inner: FolderLocation {
                    name: _,
                    value,
                    create_folder: _,
                },
                ..
            }) => value,
            Location::ReadArchive(WithKindGuard {
                inner: ReadArchiveLocation { name: _, value },
                ..
            }) => value,
            Location::WriteArchive(WithKindGuard {
                inner:
                    WriteArchiveLocation {
                        value,
                        name: _,
                        archive_type: _,
                        archive_flags: _,
                        files_flags: _,
                        archive_compressed: _,
                    },
                ..
            }) => value,
        }
    }
}

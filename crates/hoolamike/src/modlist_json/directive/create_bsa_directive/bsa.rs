use {super::*, crate::serde_type_guard, type_guard::WithTypeGuard};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct FileStateData {
    pub flip_compression: bool,
    /// index: usize
    /// Description: Index of the file in a collection.
    /// Usage: Reference files in order.
    pub index: usize,
    /// path: PathBuf
    /// Description: File system path to the file.
    /// Usage: Access the file during installation.
    pub path: MaybeWindowsPath,
}

serde_type_guard!(BSAFileStateTypeGuard, "BSAFileState, Compression.BSA");
pub type FileState = WithTypeGuard<FileStateData, BSAFileStateTypeGuard>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct DirectiveStateData {
    pub archive_flags: u32,
    pub file_flags: u16,
    /// header_magic: String
    /// Description: Magic number or signature in the file header.
    /// Usage: Verify file format before processing.
    pub magic: String,
    /// version: u64
    /// Description: Version number of the directive or file format.
    /// Usage: Ensure compatibility with processing routines.
    pub version: u64,
}

serde_type_guard!(BSADirectiveStateTypeGuard, "BSAState, Compression.BSA");
pub type DirectiveState = WithTypeGuard<DirectiveStateData, BSADirectiveStateTypeGuard>;

test_example!(
    r#"
{
    "$type": "BSAState, Compression.BSA",
    "ArchiveFlags": 3,
    "FileFlags": 0,
    "Magic": "BSA\u0000",
    "Version": 105
}
   "#,
    test_directive_state_1,
    DirectiveState
);

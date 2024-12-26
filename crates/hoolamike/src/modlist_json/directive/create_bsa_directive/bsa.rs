use {super::*, crate::serde_type_guard, type_guard::WithTypeGuard};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileStateData {
    flip_compression: bool,
    /// index: usize
    /// Description: Index of the file in a collection.
    /// Usage: Reference files in order.
    index: usize,
    /// path: PathBuf
    /// Description: File system path to the file.
    /// Usage: Access the file during installation.
    path: MaybeWindowsPath,
}

serde_type_guard!(BSAFileStateTypeGuard, "BSAFileState, Compression.BSA");
pub type FileState = WithTypeGuard<FileStateData, BSAFileStateTypeGuard>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectiveStateData {
    archive_flags: u32,
    file_flags: u32,
    /// header_magic: String
    /// Description: Magic number or signature in the file header.
    /// Usage: Verify file format before processing.
    magic: String,
    /// version: u64
    /// Description: Version number of the directive or file format.
    /// Usage: Ensure compatibility with processing routines.
    version: u64,
}

serde_type_guard!(BSADirectiveStateTypeGuard, "BSAState, Compression.BSA");
pub type DirectiveState = WithTypeGuard<DirectiveStateData, BSADirectiveStateTypeGuard>;

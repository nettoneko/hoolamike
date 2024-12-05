use {super::*, nonempty::NonEmpty};

/// typically starts with an archive hash and then goes on recursively
/// with paths within those archives
/// BONUS_POINTS: try working with it without fully  extracting the nested archives
#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveHashPath(NonEmpty<String>);

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct CreateBSADirective {
    /// hash: String
    /// Description: Hash of the file involved in the directive.
    /// Usage: Verify file integrity before processing.
    pub hash: String,
    /// size: u64
    /// Description: Size of the file.
    /// Usage: For validation and progress tracking.
    pub size: u64,
    /// to: String
    /// Description: Destination path for the directive's output.
    /// Usage: Where to place extracted or processed files.
    pub to: String,
    #[serde(rename = "TempID")]
    /// temp_id: Option<String> (renamed from TempID)
    /// Description: Temporary identifier used during processing.
    /// Usage: Track temporary files or operations.
    pub temp_id: String,
    /// file_states: Option<Vec<FileState>>
    /// Description: Details about the state of files involved in the directive.
    /// Usage: Handle files according to their specific properties.
    pub file_states: Vec<FileState>,
    /// state: Option<DirectiveState>
    /// Description: Additional metadata about the directive's state.
    /// Usage: Process directives accurately based on their state.
    pub state: DirectiveState,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct FromArchiveDirective {
    /// hash: String
    /// Description: Hash of the file involved in the directive.
    /// Usage: Verify file integrity before processing.
    pub hash: String,
    /// size: u64
    /// Description: Size of the file.
    /// Usage: For validation and progress tracking.
    pub size: u64,
    /// to: String
    /// Description: Destination path for the directive's output.
    /// Usage: Where to place extracted or processed files.
    pub to: String,
    /// archive_hash_path: Option<Vec<String>>
    /// Description: Paths within an archive, identified by their hashes.
    /// Usage: Locate specific files inside archives.
    pub archive_hash_path: ArchiveHashPath,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct InlineFileDirective {
    /// hash: String
    /// Description: Hash of the file involved in the directive.
    /// Usage: Verify file integrity before processing.
    pub hash: String,
    /// size: u64
    /// Description: Size of the file.
    /// Usage: For validation and progress tracking.
    pub size: u64,
    #[serde(rename = "SourceDataID")]
    /// source_data_id: Option<String> (renamed from SourceDataID)
    /// Description: Identifier linking to the source data.
    /// Usage: May be used internally to reference data.
    pub source_data_id: uuid::Uuid,
    /// to: String
    /// Description: Destination path for the directive's output.
    /// Usage: Where to place extracted or processed files.
    pub to: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct PatchedFromArchiveDirective {
    /// hash: String
    /// Description: Hash of the file involved in the directive.
    /// Usage: Verify file integrity before processing.
    pub hash: String,
    /// size: u64
    /// Description: Size of the file.
    /// Usage: For validation and progress tracking.
    pub size: u64,
    #[serde(rename = "SourceDataID")]
    /// source_data_id: Option<String> (renamed from SourceDataID)
    /// Description: Identifier linking to the source data.
    /// Usage: May be used internally to reference data.
    pub source_data_id: Option<uuid::Uuid>,
    /// to: String
    /// Description: Destination path for the directive's output.
    /// Usage: Where to place extracted or processed files.
    pub to: String,
    /// archive_hash_path: Option<Vec<String>>
    /// Description: Paths within an archive, identified by their hashes.
    /// Usage: Locate specific files inside archives.
    pub archive_hash_path: ArchiveHashPath,
    /// from_hash: Option<String>
    /// Description: Hash of the source file within an archive.
    /// Usage: Verify the correct source file is used.
    pub from_hash: String,
    #[serde(rename = "PatchID")]
    /// patch_id: Option<String> (renamed from PatchID)
    /// Description: Identifier for a patch operation.
    /// Usage: Apply the correct patch during installation.
    pub patch_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct RemappedInlineFileDirective {
    /// hash: String
    /// Description: Hash of the file involved in the directive.
    /// Usage: Verify file integrity before processing.
    pub hash: String,
    /// size: u64
    /// Description: Size of the file.
    /// Usage: For validation and progress tracking.
    pub size: u64,
    #[serde(rename = "SourceDataID")]
    /// source_data_id: Option<String> (renamed from SourceDataID)
    /// Description: Identifier linking to the source data.
    /// Usage: May be used internally to reference data.
    pub source_data_id: uuid::Uuid,
    /// to: String
    /// Description: Destination path for the directive's output.
    /// Usage: Where to place extracted or processed files.
    pub to: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct TransformedTextureDirective {
    /// hash: String
    /// Description: Hash of the file involved in the directive.
    /// Usage: Verify file integrity before processing.
    pub hash: String,
    /// size: u64
    /// Description: Size of the file.
    /// Usage: For validation and progress tracking.
    pub size: u64,
    /// image_state: Option<ImageState>
    /// Description: Contains image-specific information if the directive deals with images.
    /// Usage: Process images correctly based on their properties.
    pub image_state: ImageState,
    /// to: String
    /// Description: Destination path for the directive's output.
    /// Usage: Where to place extracted or processed files.
    pub to: String,
    /// archive_hash_path: Option<Vec<String>>
    /// Description: Paths within an archive, identified by their hashes.
    /// Usage: Locate specific files inside archives.
    pub archive_hash_path: ArchiveHashPath,
}

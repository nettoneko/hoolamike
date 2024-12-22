use {
    crate::{install_modlist::download_cache::to_base_64_from_u64, utils::MaybeWindowsPath},
    serde::{Deserialize, Serialize},
    std::hash::Hasher,
    tap::prelude::*,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct Modlist {
    /// archives: Vec<Archive>
    /// Description: A list of archives (mod files) required for the modlist.
    /// Usage: You'll need to download each archive listed here.
    pub archives: Vec<Archive>,
    /// author: String
    /// Description: The name of the modlist's creator.
    /// Usage: Display or record the author's name for attribution.
    pub author: String,
    /// description: String
    /// Description: A brief description of the modlist.
    /// Usage: Show this to users to inform them about the modlist.
    pub description: String,
    /// directives: Vec<Directive>
    /// Description: Instructions on how to process the archives and install the mods.
    /// Usage: Follow these directives to install the mods correctly.
    pub directives: Vec<Directive>,
    /// game_type: String
    /// Description: The type of game the modlist is for (e.g., "Skyrim", "Fallout4").
    /// Usage: Ensure compatibility with the user's game.
    pub game_type: GameName,
    /// image: String
    /// Description: Path or URL to an image representing the modlist.
    /// Usage: Display this image in your tool's UI.
    pub image: String,
    /// is_nsfw: bool
    /// Description: Indicates if the modlist contains adult content.
    /// Usage: Warn users or enforce age restrictions as necessary.
    #[serde(rename = "IsNSFW")]
    pub is_nsfw: bool,
    /// name: String
    /// Description: The name of the modlist.
    /// Usage: Display or record the modlist's name.
    pub name: String,
    /// readme: String
    /// Description: Path or URL to a README file with detailed instructions.
    /// Usage: Provide access to the README for additional guidance.
    pub readme: String,
    /// version: String
    /// Description: The version number of the modlist.
    /// Usage: Manage updates or compatibility checks.
    pub version: String,
    /// wabbajack_version: String
    /// Description: The version of Wabbajack used to create the modlist.
    /// Usage: Ensure compatibility with your tool.
    pub wabbajack_version: String,
    /// website: String
    /// Description: The modlist's website or homepage.
    /// Usage: Provide users with a link for more information.
    pub website: String,
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct ArchiveDescriptor {
    /// hash: String
    /// Description: A hash (e.g., SHA256) of the archive file for integrity verification.
    /// Usage: Verify downloaded files to prevent corruption or tampering.
    pub hash: String,
    /// meta: String
    /// Description: Metadata about the archive, possibly including download source info.
    /// Usage: May contain details needed for downloading or processing the archive.
    pub meta: String,
    /// name: String
    /// Description: The filename of the archive.
    /// Usage: Use this when saving or referencing the archive.
    pub name: String,
    /// size: u64
    /// Description: Size of the archive in bytes.
    /// Usage: For progress tracking and verifying download completeness.
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Archive {
    #[serde(flatten)]
    pub descriptor: ArchiveDescriptor,
    /// state: State
    /// Description: Contains information about where and how to download the archive.
    /// Usage: Use the State fields to handle the download process.
    pub state: State,
}

mod type_guard;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, enum_kinds::EnumKind)]
#[serde(tag = "$type")]
#[serde(deny_unknown_fields)]
#[enum_kind(DownloadKind, derive(Serialize, Deserialize, PartialOrd, Ord, derive_more::Display,))]
pub enum State {
    #[serde(rename = "NexusDownloader, Wabbajack.Lib")]
    Nexus(NexusState),
    #[serde(rename = "GameFileSourceDownloader, Wabbajack.Lib")]
    GameFileSource(GameFileSourceState),
    #[serde(rename = "GoogleDriveDownloader, Wabbajack.Lib")]
    GoogleDrive(GoogleDriveState),
    #[serde(rename = "HttpDownloader, Wabbajack.Lib")]
    Http(HttpState),
    #[serde(rename = "ManualDownloader, Wabbajack.Lib")]
    Manual(ManualState),
    #[serde(rename = "WabbajackCDNDownloader+State, Wabbajack.Lib")]
    WabbajackCDN(WabbajackCDNDownloaderState),
}

impl State {
    pub fn kind(&self) -> DownloadKind {
        DownloadKind::from(self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct HttpState {
    #[serde(default)]
    pub headers: Vec<()>,
    pub url: url::Url,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct ManualState {
    pub prompt: String,
    pub url: url::Url,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct WabbajackCDNDownloaderState {
    pub url: url::Url,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct GoogleDriveState {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct GameFileSourceState {
    pub game_version: String,
    pub hash: String,
    pub game_file: MaybeWindowsPath,
    pub game: GameName,
}

#[derive(Debug, Serialize, Deserialize, Clone, derive_more::AsRef, derive_more::Display, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Constructor)]
pub struct GameName(String);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(deny_unknown_fields)]
pub struct NexusState {
    pub game_name: GameName,
    #[serde(rename = "FileID")]
    pub file_id: usize,
    #[serde(rename = "ModID")]
    pub mod_id: usize,
    pub author: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "ImageURL")]
    /// image_url: Option<String>
    /// Description: URL to an image associated with the mod.
    /// Usage: Display in your tool's UI.
    pub image_url: Option<String>,
    #[serde(rename = "IsNSFW")]
    /// is_nsfw: Option<bool> (renamed from IsNSFW)
    /// Description: Indicates if the mod contains adult content.
    /// Usage: Implement content warnings or filters.
    pub is_nsfw: bool,
    /// name: Option<String>
    /// Description: The name of the mod or archive.
    /// Usage: Display to the user or use in logs.
    pub name: String,
    /// version: Option<String>
    /// Description: The version of the mod.
    /// Usage: Ensure correct versions are downloaded.
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct UnknownState {
    /// id: Option<String>
    /// Description: An optional identifier, possibly for use with specific download sources.
    /// Usage: May be required for API calls to download the archive.
    pub id: Option<String>,
    /// name: Option<String>
    /// Description: The name of the mod or archive.
    /// Usage: Display to the user or use in logs.
    pub name: Option<String>,
    /// prompt: Option<String>
    /// Description: A message to display to the user, possibly for manual download steps.
    /// Usage: Show prompts when user action is required.
    pub prompt: Option<String>,
    /// version: Option<String>
    /// Description: The version of the mod.
    /// Usage: Ensure correct versions are downloaded.
    pub version: Option<String>,
    // #[serde(rename = "$type")]
    // /// kind: String (renamed from $type)
    // /// Description: The type of download state (e.g., "Nexus", "Manual", "Url").
    // /// Usage: Determines the method to use when downloading the archive.
    // pub kind: DownloadKind,
    /// game: Option<String>
    /// Description: The game associated with the mod.
    /// Usage: Verify compatibility.
    pub game: Option<String>,
    /// game_file: Option<String>
    /// Description: Specific game file related to the mod.
    /// Usage: May be necessary for certain mod types.
    pub game_file: Option<String>,
    /// game_version: Option<String>
    /// Description: The game version required for the mod.
    /// Usage: Check against the user's game version.
    pub game_version: Option<String>,
    /// game_name: Option<String>
    /// Description: The name of the game.
    /// Usage: For display purposes or validation.
    pub game_name: Option<GameName>,
    #[serde(rename = "ImageURL")]
    /// image_url: Option<String>
    /// Description: URL to an image associated with the mod.
    /// Usage: Display in your tool's UI.
    pub image_url: Option<String>,
    /// hash: Option<String>
    /// Description: Additional hash for verification.
    /// Usage: Use for extra integrity checks if provided.
    pub hash: Option<String>,
    /// headers: Option<Vec<String>>
    /// Description: HTTP headers required for downloading the file.
    /// Usage: Include these headers in your HTTP requests.
    pub headers: Option<Vec<String>>, // Assuming headers is a list of strings, adjust if necessary
    /// url: Option<String>
    /// Description: Direct download URL for the archive.
    /// Usage: Use this URL to download the file.
    pub url: Option<String>,
    /// author: Option<String>
    /// Description: The author of the mod.
    /// Usage: For display or attribution.
    pub author: Option<String>,
    /// description: Option<String>
    /// Description: A description of the mod.
    /// Usage: Display to the user for more context.
    pub description: Option<String>,
    #[serde(rename = "FileID")]
    /// file_id: Option<usize> (renamed from FileID)
    /// Description: Specific file ID from the mod hosting site (e.g., Nexus Mods).
    /// Usage: Needed for API calls to download from mod hosting sites.
    pub file_id: Option<usize>,
    #[serde(rename = "IsNSFW")]
    /// is_nsfw: Option<bool> (renamed from IsNSFW)
    /// Description: Indicates if the mod contains adult content.
    /// Usage: Implement content warnings or filters.
    pub is_nsfw: Option<bool>,
    #[serde(rename = "ModID")]
    /// mod_id: Option<usize> (renamed from ModID)
    /// Description: Mod ID from the hosting site.
    /// Usage: Required for downloading from specific mod repositories.
    pub mod_id: Option<usize>,
}

pub mod directive;

#[derive(Debug, Serialize, Deserialize, enum_kinds::EnumKind)]
#[serde(tag = "$type")]
#[serde(deny_unknown_fields)]
#[enum_kind(DirectiveKind, derive(Serialize, Deserialize, PartialOrd, Ord, derive_more::Display, Hash, clap::ValueEnum))]
pub enum Directive {
    CreateBSA(directive::CreateBSADirective),
    FromArchive(directive::FromArchiveDirective),
    InlineFile(directive::InlineFileDirective),
    PatchedFromArchive(directive::PatchedFromArchiveDirective),
    RemappedInlineFile(directive::RemappedInlineFileDirective),
    TransformedTexture(directive::TransformedTextureDirective),
}

impl Directive {
    pub fn directive_hash(&self) -> String {
        serde_json::to_string(self).unwrap().pipe(|out| {
            let mut hasher = xxhash_rust::xxh64::Xxh64::new(0);
            hasher.update(out.as_bytes());
            hasher.finish().pipe(to_base_64_from_u64)
        })
    }
}

impl Directive {
    pub fn directive_kind(&self) -> DirectiveKind {
        DirectiveKind::from(self)
    }
}

pub mod image_format;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct ImageState {
    /// format: String
    /// Description: Image file format (e.g., "DDS", "PNG").
    /// Usage: Handle the image appropriately during installation.
    pub format: self::image_format::DXGIFormat,
    /// height: u64
    /// Description: Height of the image in pixels.
    /// Usage: May be needed for processing or validation.
    pub height: u32,
    /// mip_levels: u64
    /// Description: Number of mipmap levels in the image.
    /// Usage: Important for rendering and performance.
    pub mip_levels: u32,
    /// perceptual_hash: String
    /// Description: Hash representing the image's visual content.
    /// Usage: Detect duplicate or similar images.
    pub perceptual_hash: String,
    /// width: u64
    /// Description: Width of the image in pixels.
    /// Usage: May be needed for processing or validation.
    pub width: u32,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, enum_kinds::EnumKind)]
#[serde(tag = "$type")]
#[serde(deny_unknown_fields)]
#[enum_kind(DirectiveStateKind, derive(Serialize, Deserialize, PartialOrd, Ord, derive_more::Display,))]
pub enum DirectiveState {
    #[serde(rename = "BA2State, Compression.BSA")]
    #[serde(rename_all = "PascalCase")]
    CompressionBsa {
        /// has_name_table: bool
        /// Description: Indicates if the file contains a name table.
        /// Usage: Important for processing certain file formats.
        has_name_table: bool,
        /// header_magic: String
        /// Description: Magic number or signature in the file header.
        /// Usage: Verify file format before processing.
        header_magic: String,
        #[serde(rename = "Type")]
        /// kind: u64 (renamed from Type)
        /// Description: Numeric code representing the directive's kind.
        /// Usage: May influence processing logic.
        kind: u64,
        /// version: u64
        /// Description: Version number of the directive or file format.
        /// Usage: Ensure compatibility with processing routines.
        version: u64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct UnknownDirectiveState {
    #[serde(rename = "$type")]
    /// directive_state_type: String (renamed from $type)
    /// Description: Type of directive state.
    /// Usage: Determines special handling requirements.
    pub directive_state_type: String,
    /// has_name_table: bool
    /// Description: Indicates if the file contains a name table.
    /// Usage: Important for processing certain file formats.
    pub has_name_table: bool,
    /// header_magic: String
    /// Description: Magic number or signature in the file header.
    /// Usage: Verify file format before processing.
    pub header_magic: String,
    #[serde(rename = "Type")]
    /// kind: u64 (renamed from Type)
    /// Description: Numeric code representing the directive's kind.
    /// Usage: May influence processing logic.
    pub kind: u64,
    /// version: u64
    /// Description: Version number of the directive or file format.
    /// Usage: Ensure compatibility with processing routines.
    pub version: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct BA2DX10Entry {
    /// dir_hash: u64
    /// Description: Hash of the directory path.
    /// Usage: Verify file locations or detect conflicts.
    pub dir_hash: u32,
    /// chunk_hdr_len: Option<u64>
    /// Description: Length of the chunk header.
    /// Usage: Needed when processing files split into chunks.
    pub chunk_hdr_len: u64,
    /// chunks: Option<Vec<BA2DX10EntryChunk>>
    /// Description: List of chunks if the file is divided.
    /// Usage: Reassemble or process each chunk correctly.
    pub chunks: Vec<BA2DX10EntryChunk>,
    /// num_mips: Option<u64>
    /// Description: Number of mipmap levels in a texture.
    /// Usage: Important for texture processing.
    pub num_mips: u8,
    /// pixel_format: Option<u64>
    /// Description: Numeric code for the image's pixel format.
    /// Usage: Handle image data accurately.
    pub pixel_format: u8,
    /// tile_mode: Option<u64>
    /// Description: Tiling mode used in the texture.
    /// Usage: For rendering or processing textures.
    pub tile_mode: u8,
    #[serde(rename = "Unk8")]
    /// unk_8: Option<u8> (renamed from Unk8)
    /// Description: An unknown or unused field.
    /// Usage: May be ignored unless specified.
    pub unk_8: u8,
    /// extension: String
    /// Description: File extension (e.g., "dds", "nif").
    /// Usage: Determine how to process the file.
    pub extension: String,
    /// height: Option<u64>
    /// Description: Height of an image file.
    /// Usage: For image processing.
    pub height: u16,
    /// width: Option<u64>
    /// Description: Width of an image file.
    /// Usage: For image processing.
    pub width: u16,
    /// is_cube_map: Option<u8>
    /// Description: Indicates if the texture is a cube map.
    /// Usage: Special handling for cube maps in rendering.
    pub is_cube_map: u8,
    /// index: usize
    /// Description: Index of the file in a collection.
    /// Usage: Reference files in order.
    pub index: usize,
    /// name_hash: u64
    /// Description: Hash of the file name.
    /// Usage: Quickly compare or locate files.
    pub name_hash: u32,
    /// path: PathBuf
    /// Description: File system path to the file.
    /// Usage: Access the file during installation.
    pub path: MaybeWindowsPath,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, enum_kinds::EnumKind)]
#[serde(tag = "$type")]
#[serde(deny_unknown_fields)]
#[enum_kind(FileStateKind, derive(Serialize, Deserialize, PartialOrd, Ord, derive_more::Display,))]
pub enum FileState {
    #[serde(rename_all = "PascalCase")]
    BA2File {
        /// align: u64
        /// Description: Alignment requirement in bytes.
        /// Usage: Ensure correct alignment during file operations.
        align: u64,
        /// compressed: Option<bool>
        /// Description: Indicates if the file is compressed.
        /// Usage: Decompress if necessary during installation.
        compressed: bool,
        /// dir_hash: u64
        /// Description: Hash of the directory path.
        /// Usage: Verify file locations or detect conflicts.
        dir_hash: u32,
        /// extension: String
        /// Description: File extension (e.g., "dds", "nif").
        /// Usage: Determine how to process the file.
        extension: String,
        /// flags: u64
        /// Description: Additional flags for file properties.
        /// Usage: Influence processing based on flag values.
        flags: u64,
        /// index: usize
        /// Description: Index of the file in a collection.
        /// Usage: Reference files in order.
        index: usize,
        /// name_hash: u64
        /// Description: Hash of the file name.
        /// Usage: Quickly compare or locate files.
        name_hash: u32,
        /// path: PathBuf
        /// Description: File system path to the file.
        /// Usage: Access the file during installation.
        path: MaybeWindowsPath,
    },
    BA2DX10Entry(BA2DX10Entry),
}

// #[derive(Debug, Serialize, Deserialize)]
// #[serde(deny_unknown_fields)]
// #[serde(rename_all = "PascalCase")]
// pub struct UnknownFileState {
//     #[serde(rename = "$type")]
//     /// file_state_type: String (renamed from $type)
//     /// Description: Specifies the file's processing requirements.
//     /// Usage: Handle the file appropriately based on its type.
//     pub file_state_type: String,
//     /// align: Option<u64>
//     /// Description: Alignment requirement in bytes.
//     /// Usage: Ensure correct alignment during file operations.
//     pub align: Option<u64>,
//     /// compressed: Option<bool>
//     /// Description: Indicates if the file is compressed.
//     /// Usage: Decompress if necessary during installation.
//     pub compressed: Option<bool>,
//     /// dir_hash: u64
//     /// Description: Hash of the directory path.
//     /// Usage: Verify file locations or detect conflicts.
//     pub dir_hash: u64,
//     /// chunk_hdr_len: Option<u64>
//     /// Description: Length of the chunk header.
//     /// Usage: Needed when processing files split into chunks.
//     pub chunk_hdr_len: Option<u64>,
//     /// chunks: Option<Vec<FileStateChunk>>
//     /// Description: List of chunks if the file is divided.
//     /// Usage: Reassemble or process each chunk correctly.
//     pub chunks: Option<Vec<BA2DX10EntryChunk>>,
//     /// num_mips: Option<u64>
//     /// Description: Number of mipmap levels in a texture.
//     /// Usage: Important for texture processing.
//     pub num_mips: Option<u64>,
//     /// pixel_format: Option<u64>
//     /// Description: Numeric code for the image's pixel format.
//     /// Usage: Handle image data accurately.
//     pub pixel_format: Option<u64>,
//     /// tile_mode: Option<u64>
//     /// Description: Tiling mode used in the texture.
//     /// Usage: For rendering or processing textures.
//     pub tile_mode: Option<u64>,
//     #[serde(rename = "Unk8")]
//     /// unk_8: Option<u8> (renamed from Unk8)
//     /// Description: An unknown or unused field.
//     /// Usage: May be ignored unless specified.
//     pub unk_8: Option<u8>,
//     /// extension: String
//     /// Description: File extension (e.g., "dds", "nif").
//     /// Usage: Determine how to process the file.
//     pub extension: String,
//     /// height: Option<u64>
//     /// Description: Height of an image file.
//     /// Usage: For image processing.
//     pub height: Option<u64>,
//     /// width: Option<u64>
//     /// Description: Width of an image file.
//     /// Usage: For image processing.
//     pub width: Option<u64>,
//     /// is_cube_map: Option<u8>
//     /// Description: Indicates if the texture is a cube map.
//     /// Usage: Special handling for cube maps in rendering.
//     pub is_cube_map: Option<u8>,
//     /// flags: Option<u64>
//     /// Description: Additional flags for file properties.
//     /// Usage: Influence processing based on flag values.
//     pub flags: Option<u64>,
//     /// index: usize
//     /// Description: Index of the file in a collection.
//     /// Usage: Reference files in order.
//     pub index: usize,
//     /// name_hash: u64
//     /// Description: Hash of the file name.
//     /// Usage: Quickly compare or locate files.
//     pub name_hash: u64,
//     /// path: PathBuf
//     /// Description: File system path to the file.
//     /// Usage: Access the file during installation.
//     pub path: PathBuf,
// }

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "PascalCase")]
pub struct BA2DX10EntryChunk {
    /// align: u64
    /// Description: Alignment requirement for the chunk.
    /// Usage: Ensure correct alignment when reassembling.
    pub align: u64,
    /// compressed: bool
    /// Description: Indicates if the chunk is compressed.
    /// Usage: Decompress as needed.
    pub compressed: bool,
    /// end_mip: u64
    /// Description: Ending mipmap level for this chunk.
    /// Usage: For texture processing.
    pub end_mip: u64,
    /// full_sz: u64
    /// Description: Full size of the chunk in bytes.
    /// Usage: For progress tracking and validation.
    pub full_sz: u64,
    /// start_mip: u64
    /// Description: Starting mipmap level for this chunk.
    /// Usage: For texture processing.
    pub start_mip: u64,
}

pub mod parsing_helpers {
    use {
        anyhow::{Context, Result},
        itertools::Itertools,
        serde_json::Value,
        std::collections::BTreeMap,
        tap::prelude::*,
        tracing::info,
    };

    #[allow(dead_code)]
    #[derive(Debug)]
    enum ValueSummary<'a> {
        Map { fields: BTreeMap<&'a str, Self> },
        Array { first_element: Option<Box<Self>>, len: usize },
        Other(&'a serde_json::Value),
    }

    pub fn validate_modlist_file(input: &str) -> Result<()> {
        input
            .tap(|input| {
                info!("file is {} bytes long", input.bytes().len());
            })
            .pipe_as_ref(serde_json::from_str::<Value>)
            .context("bad json")
            .and_then(|node| serde_json::to_string_pretty(&node).context("serializing"))
            .and_then(move |pretty_input| {
                serde_json::from_str::<crate::modlist_json::Modlist>(&pretty_input)
                    .pipe(|res| match res.as_ref() {
                        Ok(_) => res.context(""),
                        Err(e) => e.line().pipe(|line| {
                            res.with_context(|| {
                                pretty_input
                                    .lines()
                                    .enumerate()
                                    .map(|(idx, line)| format!("{}. {line}", idx + 1))
                                    .skip(line - 20)
                                    .take(40)
                                    .join("\n")
                            })
                        }),
                    })
                    .context("bad modlist")
            })
            .map(|_| ())
    }

    #[allow(unexpected_cfgs)]
    #[cfg(test)]
    mod ad_hoc_test {
        use super::*;

        #[allow(dead_code)]
        fn summarize_node(node: &Value) -> ValueSummary {
            match node {
                Value::Array(vec) => ValueSummary::Array {
                    first_element: vec.first().map(summarize_node).map(Box::new),
                    len: vec.len(),
                },
                Value::Object(map) => ValueSummary::Map {
                    fields: map
                        .iter()
                        .map(|(key, value)| (key.as_str(), summarize_node(value)))
                        .collect(),
                },
                other => ValueSummary::Other(other),
            }
        }

        // #[cfg(ignore)]
        #[test_log::test]
        fn test_wasteland_reborn() -> anyhow::Result<()> {
            use super::*;

            include_str!("../../../../wasteland-reborn/test/modlist").pipe(validate_modlist_file)
        }
    }
}

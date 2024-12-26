use {
    super::{try_optimize_memory_mapping, PathReadWrite},
    crate::modlist_json::BA2DX10Entry,
    anyhow::{Context, Result},
    ba2::{
        tes4::{Archive, ArchiveKey, File, FileHash, FileReadOptions, Hash},
        Borrowed,
        CompressionResult,
        ReaderWithOptions,
    },
    std::path::PathBuf,
    tap::prelude::*,
};

pub(super) struct LazyArchiveFile {
    file: memmap2::Mmap,
    read_options: FileReadOptions,
}

impl LazyArchiveFile {
    pub fn new(from_file: &std::fs::File, compressed: bool) -> Result<Self> {
        // SAFETY: do not touch that file while it's opened please
        unsafe { memmap2::Mmap::map(from_file) }
            .context("creating file")
            .tap_ok(super::try_optimize_memory_mapping)
            .map(|file| Self {
                file,
                read_options: FileReadOptions::builder()
                    .compression_result(if compressed {
                        CompressionResult::Compressed
                    } else {
                        CompressionResult::Decompressed
                    })
                    .build(),
            })
    }
    fn as_bytes(&self) -> &[u8] {
        &self.file[..]
    }
    pub fn as_archive_file(&self) -> Result<File<'_>> {
        File::read(Borrowed(self.as_bytes()), &self.read_options)
            .context("reading file using memory mapping")
            .context("building bsa archive file")
    }
}

// pub(super) fn create_key<'a>(extension: &str, name_hash: u32, dir_hash: u32) -> Result<ArchiveKey<'a>> {
//     Archive::new().insert(, )
//     extension
//         .as_bytes()
//         .split_at_checked(4)
//         .context("bad extension_size")
//         .and_then(|(bytes, rest)| {
//             rest.is_empty()
//                 .then_some(bytes)
//                 .context("extension too long")
//         })
//         .and_then(|extension| {
//             extension
//                 .to_vec()
//                 .try_conv::<[u8; 4]>()
//                 .map_err(|bad_size| anyhow::anyhow!("validating size: bad size: {bad_size:?}"))
//         })
//         .map(u32::from_le_bytes)
//         .map(|extension| Hash {
//             last: todo!(),
//             last2: todo!(),
//             length: todo!(),
//             first: todo!(),
//             crc: todo!(),
//         })
//         .map(|key_hash| key_hash.conv::<FileHash>().conv::<ArchiveKey>())
//         .context("creating key")
// }

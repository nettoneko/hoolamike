use {
    super::{try_optimize_memory_mapping, PathReadWrite},
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

use {
    super::{count_progress_style, try_optimize_memory_mapping, PathReadWrite},
    crate::{
        modlist_json::{
            directive::create_bsa_directive::{
                ba2::{BA2DX10Entry, BA2FileEntry, DirectiveStateData, FileState},
                Ba2,
            },
            type_guard::WithTypeGuard,
            BA2DX10EntryChunk,
        },
        utils::MaybeWindowsPath,
    },
    anyhow::{Context, Result},
    ba2::{
        fo4::{
            Archive,
            ArchiveKey,
            ArchiveOptions,
            ChunkCompressionOptions,
            CompressionFormat,
            CompressionLevel,
            File,
            FileHeader,
            FileReadOptions,
            Format,
            Version as ArchiveVersion,
        },
        Borrowed,
        CompressionResult,
        ReaderWithOptions,
    },
    rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
    std::path::PathBuf,
    tap::prelude::*,
    tracing::{info_span, instrument},
    tracing_indicatif::span_ext::IndicatifSpanExt,
};

#[derive(derive_more::From)]
enum LazyArchiveKind {
    File(LazyArchiveFile<BA2FileEntry>),
    DX10(LazyArchiveFile<BA2DX10Entry>),
}

impl LazyArchiveKind {
    fn as_archive_file(&self) -> Result<File<'_>> {
        match self {
            LazyArchiveKind::File(i) => i.as_archive_file(),
            LazyArchiveKind::DX10(i) => i.as_archive_file(),
        }
    }
}

pub(super) struct LazyArchiveFile<Directive> {
    file: memmap2::Mmap,
    directive: Directive,
}

impl<Directive> LazyArchiveFile<Directive> {
    pub fn new(from_file: &std::fs::File, directive: Directive) -> Result<Self> {
        unsafe { memmap2::Mmap::map(from_file) }
            .context("creating file")
            .tap_ok(try_optimize_memory_mapping)
            .map(|file| Self { file, directive })
    }
    fn as_bytes(&self) -> &[u8] {
        &self.file[..]
    }
}

impl LazyArchiveFile<BA2FileEntry> {
    fn as_archive_file(&self) -> Result<File<'_>> {
        File::read(
            Borrowed(self.as_bytes()),
            &FileReadOptions::builder()
                .format(Format::GNRL)
                .compression_format(CompressionFormat::Zip)
                .compression_level(CompressionLevel::FO4)
                .compression_result(if self.directive.compressed {
                    CompressionResult::Compressed
                } else {
                    CompressionResult::Decompressed
                })
                .build(),
        )
        .context("reading file using memory mapping")
        .context("building bsa archive file")
    }
}

impl LazyArchiveFile<BA2DX10Entry> {
    fn as_archive_file(&self) -> Result<File<'_>> {
        File::read(
            Borrowed(self.as_bytes()),
            &FileReadOptions::builder()
                .format(Format::DX10)
                .compression_result(CompressionResult::Decompressed)
                .build(),
        )
        .context("reading file using memory mapping")
        .context("building bsa archive file")
        .and_then(|mut file| {
            let res = file
                .iter_mut()
                .zip(&self.directive.chunks)
                .try_for_each(|(chunk, BA2DX10EntryChunk { compressed, .. })| {
                    if *compressed {
                        *chunk = chunk
                            .compress(
                                &ChunkCompressionOptions::builder()
                                    .compression_format(CompressionFormat::Zip)
                                    .compression_level(CompressionLevel::FO4)
                                    .build(),
                            )
                            .context("compressing chunk")?
                    }
                    Ok(())
                });
            res.map(move |_| file)
        })
    }
}

pub(super) fn create_key<'a>(extension: &str, name_hash: u32, dir_hash: u32) -> Result<ArchiveKey<'a>> {
    extension
        .as_bytes()
        .split_at_checked(4)
        .context("bad extension_size")
        .and_then(|(bytes, rest)| {
            rest.is_empty()
                .then_some(bytes)
                .context("extension too long")
        })
        .and_then(|extension| {
            extension
                .to_vec()
                .try_conv::<[u8; 4]>()
                .map_err(|bad_size| anyhow::anyhow!("validating size: bad size: {bad_size:?}"))
        })
        .map(u32::from_le_bytes)
        .map(|extension| ba2::fo4::Hash {
            extension,
            file: name_hash,
            directory: dir_hash,
        })
        .map(|key_hash| key_hash.conv::<ba2::fo4::FileHash>().conv::<ArchiveKey>())
        .context("creating key")
}

#[instrument(skip(handle_archive))]
pub fn create_archive<F: FnOnce(&Archive<'_>, ArchiveOptions, MaybeWindowsPath) -> Result<()>>(
    temp_bsa_dir: PathBuf,
    Ba2 {
        hash: _,
        size: _,
        to,
        temp_id,
        file_states,
        state:
            WithTypeGuard {
                inner:
                    DirectiveStateData {
                        has_name_table,
                        header_magic: _,
                        kind: _,
                        version,
                    },
                ..
            },
    }: Ba2,
    handle_archive: F,
) -> Result<()> {
    let version: ArchiveVersion = match version {
        1 => ArchiveVersion::v1,
        2 => ArchiveVersion::v2,
        3 => ArchiveVersion::v3,
        7 => ArchiveVersion::v7,
        8 => ArchiveVersion::v8,
        other => anyhow::bail!("unsuppored archive version: {other}"),
    };
    let temp_id_dir = temp_bsa_dir.join(temp_id);
    let reading_bsa_entries = info_span!("creating_bsa_entries", count=%file_states.len())
        .entered()
        .tap(|pb| {
            pb.pb_set_style(&count_progress_style());
            pb.pb_set_length(file_states.len() as _);
        });
    file_states
        .into_par_iter()
        .map(move |file_state| match file_state {
            FileState::BA2File(ba2_file_entry) => temp_id_dir
                .join(ba2_file_entry.path.clone().into_path())
                .pipe(|path| path.open_file_read())
                .and_then(|(_path, file)| LazyArchiveFile::new(&file, ba2_file_entry.clone()).map(LazyArchiveKind::from))
                .and_then(|file| {
                    ba2_file_entry.pipe(
                        |BA2FileEntry {
                             dir_hash,
                             extension,
                             name_hash,
                             ..
                         }| create_key(&extension, name_hash, dir_hash).map(|key| (key, file)),
                    )
                }),
            FileState::BA2DX10Entry(ba2_dx10_entry) => temp_id_dir
                .join(ba2_dx10_entry.path.clone().into_path())
                .open_file_read()
                .and_then(|(path, file)| {
                    LazyArchiveFile::new(&file, ba2_dx10_entry.clone())
                        .with_context(|| format!("opening file at [{path:?}]"))
                        .map(LazyArchiveKind::from)
                })
                .and_then(|file| {
                    ba2_dx10_entry.pipe(
                        |BA2DX10Entry {
                             dir_hash,
                             extension,
                             name_hash,
                             ..
                         }| { create_key(&extension, name_hash, dir_hash).map(|key| (key, file)) },
                    )
                }),
        })
        .inspect(|_| reading_bsa_entries.pb_inc(1))
        .collect::<Result<Vec<_>>>()
        .and_then(|entries| {
            let building_archive = info_span!("building_archive").entered().tap(|pb| {
                pb.pb_set_style(&count_progress_style());
                pb.pb_set_length(entries.len() as _);
            });
            entries.pipe_ref(|entries| {
                entries
                    .par_iter()
                    .map(|(key, file)| {
                        file.as_archive_file().map(|file| {
                            building_archive.pb_inc(1);
                            (key, file)
                        })
                    })
                    .collect::<Result<Vec<_>>>()
                    .and_then(|entries| {
                        entries
                            .first()
                            .map(|(_, file)| match file.header {
                                FileHeader::GNRL => Format::GNRL,
                                FileHeader::DX10(_) => Format::DX10,
                                FileHeader::GNMF(_) => Format::GNMF,
                            })
                            .unwrap_or_default()
                            .pipe(|format| ArchiveOptions::builder().format(format))
                            .pipe(|options| {
                                entries
                                    .into_iter()
                                    .fold(Archive::new(), |acc, (key, file)| {
                                        acc.tap_mut(|acc| {
                                            acc.insert(key.clone(), file);
                                        })
                                    })
                                    .pipe(|archive| (archive, options.version(version).strings(has_name_table).build()))
                                    .pipe(|(archive, options)| handle_archive(&archive, options, to))
                            })
                    })
                    .context("creating BA2 (fallout4/starfield) archive")
            })
        })
}

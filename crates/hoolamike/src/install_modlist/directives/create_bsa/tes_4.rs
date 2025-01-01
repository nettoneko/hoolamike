use {
    super::{count_progress_style, PathReadWrite},
    crate::{
        modlist_json::{
            directive::create_bsa_directive::{
                bsa::{DirectiveStateData, FileStateData},
                Bsa,
            },
            type_guard::WithTypeGuard,
        },
        utils::MaybeWindowsPath,
    },
    anyhow::{Context, Result},
    ba2::{
        tes4::{Archive, ArchiveFlags, ArchiveKey, ArchiveOptions, ArchiveTypes, Directory, DirectoryKey, File, FileReadOptions, Version},
        Borrowed,
        CompressionResult,
        ReaderWithOptions,
    },
    rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
    std::path::PathBuf,
    tap::prelude::*,
    tracing::info_span,
    tracing_indicatif::span_ext::IndicatifSpanExt,
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

pub(super) fn create_key<'a>(path: MaybeWindowsPath) -> Result<(ArchiveKey<'a>, DirectoryKey<'a>)> {
    // path.0
    //     .into_bytes()
    //     .conv::<BString>()
    //     .conv::<ArchiveKey>()
    //     .pipe(Ok)
    let join_delimiter = None
        .or_else(|| path.0.contains(r#"\\"#).then_some(r#"\\"#))
        .or_else(|| path.0.contains(r#"/"#).then_some(r#"/"#))
        .or_else(|| path.0.contains(r#"\"#).then_some(r#"\"#))
        .unwrap_or("/");

    path.into_path().pipe_ref(|path| {
        path.file_name()
            .context("path has no file name at the end")
            .and_then(|directory_key| {
                path.parent()
                    .context("cannot insert files at root, right?")
                    .map(|archive_key| {
                        (
                            archive_key
                                .join(join_delimiter)
                                .as_os_str()
                                .as_encoded_bytes()
                                .conv::<ArchiveKey>(),
                            directory_key.as_encoded_bytes().conv::<DirectoryKey>(),
                        )
                    })
            })
    })
}

pub fn create_archive<F: FnOnce(&Archive<'_>, ArchiveOptions, MaybeWindowsPath) -> Result<()>>(
    temp_bsa_dir: PathBuf,
    Bsa {
        hash: _,
        size: _,
        to,
        temp_id,
        file_states,
        state:
            WithTypeGuard {
                inner:
                    DirectiveStateData {
                        archive_flags,
                        file_flags,
                        magic: _,
                        version,
                    },
                ..
            },
    }: Bsa,
    handle_archive: F,
) -> Result<()> {
    let version = match version {
        103 => Version::v103,
        104 => Version::v104,
        105 => Version::v105,
        other => anyhow::bail!("unsuppored version: {other}"),
    };
    let archive_flags = ArchiveFlags::from_bits(archive_flags).context("invalid flags: {archive_flags:b}")?;
    let archive_types = ArchiveTypes::from_bits(file_flags).context("invalid flags: {file_flags:b}")?;

    let temp_id_dir = temp_bsa_dir.join(temp_id);
    let reading_bsa_entries = info_span!("creating_bsa_entries", count=%file_states.len())
        .entered()
        .tap(|pb| {
            pb.pb_set_style(&count_progress_style());
            pb.pb_set_length(file_states.len() as _);
        });
    file_states
        .into_par_iter()
        .map(
            move |WithTypeGuard {
                      inner:
                          FileStateData {
                              flip_compression,
                              index: _,
                              path,
                          },
                      ..
                  }| {
                temp_id_dir
                    .join(path.clone().into_path())
                    .pipe(|path| path.open_file_read())
                    .and_then(|(_path, file)| LazyArchiveFile::new(&file, flip_compression))
                    .and_then(|file| create_key(path).map(|key| (key, file)))
            },
        )
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
                            .into_iter()
                            .fold(Archive::new(), |acc, ((archive_key, directory_key), file)| {
                                acc.tap_mut(|acc| match acc.get_mut(archive_key) {
                                    Some(directory) => {
                                        directory.insert(directory_key.clone(), file);
                                    }
                                    None => {
                                        acc.insert(
                                            archive_key.clone(),
                                            Directory::default().tap_mut(|directory| {
                                                directory.insert(directory_key.clone(), file);
                                            }),
                                        );
                                    }
                                })
                            })
                            .pipe(|archive| {
                                handle_archive(
                                    &archive,
                                    ArchiveOptions::builder()
                                        .version(version)
                                        .flags(archive_flags)
                                        .types(archive_types)
                                        .build(),
                                    to,
                                )
                            })
                    })
                    .context("creating BSA (skyrim and before) archive")
            })
        })
}

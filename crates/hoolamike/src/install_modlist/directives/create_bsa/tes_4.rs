use {
    super::{count_progress_style, PathReadWrite},
    crate::{
        modlist_json::{
            directive::create_bsa_directive::bsa::{self, Bsa, DirectiveStateData, FileStateData},
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
    std::{ffi::OsStr, path::PathBuf},
    tap::prelude::*,
    tracing::{debug, info_span, instrument},
    tracing_indicatif::span_ext::IndicatifSpanExt,
};

#[derive(Debug)]
pub struct LazyArchiveFile<Directive> {
    file: memmap2::Mmap,
    directive: Directive,
}

impl<Directive: std::fmt::Debug> LazyArchiveFile<Directive> {
    #[instrument]
    pub fn new(from_file: &std::fs::File, directive: Directive) -> Result<Self> {
        // SAFETY: do not touch that file while it's opened please
        debug!("creating file");
        unsafe { memmap2::Mmap::map(from_file) }
            .context("creating file")
            .tap_ok(super::try_optimize_memory_mapping)
            .map(|file| Self { file, directive })
    }
    fn as_bytes(&self) -> &[u8] {
        &self.file[..]
    }
}

impl LazyArchiveFile<FileStateData> {
    #[instrument]
    pub fn as_archive_file(&self) -> Result<File<'_>> {
        self.directive.pipe_ref(
            |FileStateData {
                 flip_compression: _,
                 index: _,
                 path: _,
             }| {
                File::read(
                    Borrowed(self.as_bytes()),
                    &FileReadOptions::builder()
                        .version(Version::SSE)
                        .compression_result(CompressionResult::Compressed)
                        .build(),
                )
                .context("reading file using memory mapping")
                .context("building bsa archive file")
                .tap_ok(|file| tracing::debug!(size=%file.len(), "loaded file"))
            },
        )
    }
}

#[instrument]
pub fn create_key<'a>(path: MaybeWindowsPath) -> Result<(ArchiveKey<'a>, DirectoryKey<'a>)> {
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
                    .and_then(|archive_key| {
                        (
                            archive_key
                                .iter()
                                .map(|os_str| os_str.to_owned())
                                .reduce(|mut acc, next| {
                                    acc.push(join_delimiter.pipe(OsStr::new));
                                    acc.push(next);
                                    acc
                                })
                                .context("empty path?")
                                .map(|path| {
                                    path.tap(|path| {
                                        tracing::debug!("deriving archive key  for {path:?}");
                                    })
                                    .as_encoded_bytes()
                                    .conv::<ArchiveKey>()
                                })
                                .context("encoding directory key")?,
                            directory_key
                                .tap(|directory_key| {
                                    tracing::debug!("deriving direcotry key for {directory_key:?}");
                                })
                                .as_encoded_bytes()
                                .conv::<DirectoryKey>(),
                        )
                            .pipe(Ok)
                    })
            })
    })
}

#[instrument(skip(handle_archive, file_states))]
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
    let archive_flags = ArchiveFlags::from_bits(archive_flags).with_context(|| format!("invalid flags: {archive_flags:b}"))?;
    let archive_types = {
        let file_flags = match file_flags {
            bsa::Either::Left(normal) => normal,
            bsa::Either::Right(weird) => {
                tracing::warn!("encountered a weird file_flags: should be 16 bit but got 32 bit. casting and hoping for the best ({weird:b})");
                weird as u16
            }
        };
        ArchiveTypes::from_bits(file_flags).with_context(|| format!("invalid file flags: {file_flags:b}"))?
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
        .map(move |WithTypeGuard { inner: file_state_data, .. }| {
            info_span!("handle_file_state", ?file_state_data).in_scope(|| {
                temp_id_dir
                    .join(file_state_data.path.clone().into_path())
                    .pipe(|path| path.open_file_read())
                    .and_then(|(path, file)| LazyArchiveFile::new(&file, file_state_data.clone()).with_context(|| format!("loading file at [{path:?}]")))
                    .and_then(|file| create_key(file_state_data.path).map(|key| (key, file)))
            })
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

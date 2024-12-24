use {
    super::*,
    crate::{
        modlist_json::{directive::CreateBSADirective, BA2DX10Entry, DirectiveState, FileState},
        progress_bars_v2::IndicatifWrapIoExt,
        utils::PathReadWrite,
    },
    ba2::{fo4::FileReadOptions, CompressableFrom, CompressionResult, ReaderWithOptions},
    indicatif::ParallelProgressIterator,
    rayon::iter::{IntoParallelIterator, ParallelIterator},
    std::{
        cell::RefCell,
        convert::identity,
        io::{Seek, Write},
    },
    tracing::debug,
};

#[derive(Clone, Debug)]
pub struct CreateBSAHandler {
    pub output_directory: PathBuf,
}

struct LazyArchiveFile {
    file: memmap2::Mmap,
    compressed: bool,
}

impl LazyArchiveFile {
    pub fn new(from_file: &std::fs::File, compressed: bool) -> Result<Self> {
        // SAFETY: do not touch that file while it's opened please
        unsafe { memmap2::Mmap::map(from_file) }
            .context("creating file")
            .map(|file| Self { file, compressed })
    }
    fn as_bytes(&self) -> &[u8] {
        &self.file[..]
    }
    pub fn as_archive_file(&self) -> Result<ba2::fo4::File<'_>> {
        match self.compressed {
            true => Err(anyhow::anyhow!("compressed is not supported")),
            false => Ok(ba2::fo4::Chunk::from_decompressed(self.as_bytes())),
        }
        .map(|chunk| [chunk].into_iter().collect::<ba2::fo4::File>())
        .context("building bsa archive file")
    }
}

fn fo4_read_options() -> ba2::fo4::FileReadOptionsBuilder {
    FileReadOptions::builder()
        .format(ba2::fo4::Format::DX10)
        .compression_format(ba2::fo4::CompressionFormat::Zip)
        .compression_level(ba2::fo4::CompressionLevel::FO4)
        .compression_result(CompressionResult::Compressed)
}

fn create_key<'a>(extension: &str, name_hash: u32, dir_hash: u32) -> Result<ba2::fo4::ArchiveKey<'a>> {
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
        .map(|key_hash| {
            key_hash
                .conv::<ba2::fo4::FileHash>()
                .conv::<ba2::fo4::ArchiveKey>()
        })
}
struct BA2DX10File<'a>(ba2::fo4::File<'a>);

impl<'a> BA2DX10File<'a> {
    fn load_dx_entry(
        temp_id_directory_path: PathBuf,
        BA2DX10Entry {
            dir_hash,
            chunk_hdr_len: _,
            chunks: _,
            num_mips: _,
            pixel_format: _,
            tile_mode: _,
            unk_8: _,
            extension,
            height,
            width,
            is_cube_map: _,
            index: _,
            name_hash,
            path,
        }: BA2DX10Entry,
    ) -> Result<(ba2::fo4::ArchiveKey<'a>, Self)> {
        temp_id_directory_path
            .join(path.into_path())
            .pipe(|source_path| {
                source_path.open_file_read().and_then(|(path, file)| {
                    ba2::fo4::File::read(
                        &file,
                        &fo4_read_options()
                            .mip_chunk_height(height.conv())
                            .mip_chunk_width(width.conv())
                            .build(),
                    )
                    .with_context(|| format!("reading file at {path:?}"))
                })
            })
            .map(Self)
            .and_then(|file| create_key(&extension, name_hash, dir_hash).map(|key| (key, file)))
    }
}

impl CreateBSAHandler {
    #[tracing::instrument(skip_all, fields(hash, size, to, temp_id), level = "INFO")]
    pub async fn handle(
        self,
        CreateBSADirective {
            hash: _,
            size,
            to,
            temp_id,
            file_states,
            state,
        }: CreateBSADirective,
    ) -> Result<u64> {
        let Self { output_directory } = self;
        tokio::task::spawn_blocking(move || match state {
            DirectiveState::CompressionBsa {
                has_name_table: _,
                header_magic: _,
                kind: _,
                version: _,
            } => {
                enum PreparedEntry<'a> {
                    Normal(LazyArchiveFile),
                    DX10(RefCell<Option<BA2DX10File<'a>>>),
                }
                let source_path = output_directory
                    .clone()
                    .join("TEMP_BSA_FILES")
                    .join(&temp_id);
                file_states
                    .into_par_iter()
                    .progress()
                    .with_prefix("[Build BSA]")
                    .with_message(format!("building fresh bethesda archive at [{}]", to.clone().into_path().display()))
                    .map(|file_state| match file_state {
                        FileState::BA2File {
                            dir_hash,
                            extension,
                            name_hash,
                            path,
                            ..
                        } => source_path
                            .join(path.into_path())
                            .pipe(|path| path.open_file_read())
                            .and_then(|(_path, file)| {
                                LazyArchiveFile::new(&file, false)
                                    .and_then(|file| create_key(&extension, name_hash, dir_hash).map(|key| (key, file.pipe(PreparedEntry::Normal))))
                            }),
                        FileState::BA2DX10Entry(ba2_dx10_entry) => BA2DX10File::load_dx_entry(source_path.clone(), ba2_dx10_entry).map(|(key, entry)| {
                            (
                                key,
                                entry
                                    .pipe(Some)
                                    .pipe(RefCell::new)
                                    .pipe(PreparedEntry::DX10),
                            )
                        }),
                    })
                    .collect::<Result<Vec<_>>>()
                    .and_then(|entries| {
                        output_directory
                            .clone()
                            .join(to.into_path())
                            .open_file_write()
                            .and_then(|(output_path, mut output)| {
                                entries.pipe_ref(|entries| {
                                    entries
                                        .iter()
                                        .try_fold(
                                            (ba2::fo4::Archive::new(), ba2::fo4::ArchiveOptions::builder()),
                                            |(mut acc, options), (key, file)| match file {
                                                PreparedEntry::Normal(file) => file.as_archive_file().map(|file| {
                                                    let options = match file.header {
                                                        ba2::fo4::FileHeader::GNRL => options.format(ba2::fo4::Format::GNRL),
                                                        ba2::fo4::FileHeader::DX10(_) => options.format(ba2::fo4::Format::DX10),
                                                        ba2::fo4::FileHeader::GNMF(_) => options.format(ba2::fo4::Format::GNMF),
                                                    };
                                                    acc.insert(key.clone(), file);
                                                    (acc, options)
                                                }),
                                                PreparedEntry::DX10(ba2_dx10_file) => ba2_dx10_file
                                                    .borrow_mut()
                                                    .take()
                                                    .context("come on")
                                                    .map(|file| {
                                                        let options = match file.0.header {
                                                            ba2::fo4::FileHeader::GNRL => options.format(ba2::fo4::Format::GNRL),
                                                            ba2::fo4::FileHeader::DX10(_) => options.format(ba2::fo4::Format::DX10),
                                                            ba2::fo4::FileHeader::GNMF(_) => options.format(ba2::fo4::Format::GNMF),
                                                        };
                                                        acc.insert(key.clone(), file.0);
                                                        (acc, options)
                                                    }),
                                            },
                                        )
                                        .and_then(|(archive, options)| {
                                            {
                                                let mut writer = tracing::Span::current().wrap_write(0, &mut output);
                                                archive.write(&mut writer, &options.build())
                                            }
                                            .context("writing the built archive")
                                            .and_then(|_| {
                                                output.rewind().context("rewinding file").and_then(|_| {
                                                    debug!("finished dumping bethesda archive");
                                                    output.flush().context("flushing").map(|_| output)
                                                })
                                            })
                                        })
                                        .with_context(|| format!("writing to [{:?}]", output_path))
                                })
                            })
                    })
            }
        })
        .instrument(tracing::debug_span!("performing_task"))
        .await
        .context("thread crashed")
        .and_then(identity)
        .map(|_| size)
    }
}

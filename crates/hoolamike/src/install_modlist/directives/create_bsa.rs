use {
    super::*,
    crate::modlist_json::{directive::CreateBSADirective, BA2DX10Entry, BA2DX10EntryChunk, DirectiveState, FileState},
    ba2::{
        fo4::{FileReadOptions, FileReadOptionsBuilder},
        CompressableFrom,
        CompressionResult,
        ReaderWithOptions,
    },
    memmap2::Mmap,
    std::{
        any::Any,
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
            true => Ok(ba2::fo4::Chunk::from_compressed(self.as_bytes(), anyhow::bail!("compressed"))),
            false => Ok(ba2::fo4::Chunk::from_decompressed(self.as_bytes())),
        }
        .map(|chunk| [chunk].into_iter().collect::<ba2::fo4::File>())
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
            chunk_hdr_len,
            chunks,
            num_mips,
            pixel_format,
            tile_mode,
            unk_8,
            extension,
            height,
            width,
            is_cube_map,
            index,
            name_hash,
            path,
        }: BA2DX10Entry,
    ) -> Result<(ba2::fo4::ArchiveKey<'a>, Self)> {
        temp_id_directory_path
            .join(path.into_path())
            .pipe(|source_path| {
                std::fs::OpenOptions::new()
                    .read(true)
                    .open(&source_path)
                    .with_context(|| format!("opening entry for readng at [{source_path:?}]"))
                    .and_then(|file| {
                        ba2::fo4::File::read(
                            &file,
                            &fo4_read_options()
                                .mip_chunk_height(height.conv())
                                .mip_chunk_width(width.conv())
                                .build(),
                        )
                        .context("reading file")
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
            hash,
            size,
            to,
            temp_id,
            file_states,
            state,
        }: CreateBSADirective,
    ) -> Result<u64> {
        let Self { output_directory } = self;
        let pb = vertical_progress_bar(size, ProgressKind::WriteBSA, indicatif::ProgressFinish::AndLeave)
            .attach_to(&PROGRESS_BAR)
            .tap_mut(|pb| {
                pb.set_message(to.clone().into_path().display().to_string());
            });
        tokio::task::spawn_blocking(move || match state {
            DirectiveState::CompressionBsa {
                has_name_table,
                header_magic,
                kind,
                version,
            } => {
                let files_pb = vertical_progress_bar(file_states.len() as _, ProgressKind::WriteBSA, indicatif::ProgressFinish::AndLeave)
                    .attach_to(&PROGRESS_BAR)
                    .tap_mut(|pb| {
                        pb.set_message(format!("handling [{}] file states", file_states.len()));
                    });

                enum PreparedEntry<'a> {
                    Normal(LazyArchiveFile),
                    DX10(RefCell<Option<BA2DX10File<'a>>>),
                }
                let source_path = output_directory
                    .clone()
                    .join("TEMP_BSA_FILES")
                    .join(&temp_id);

                file_states
                    .into_iter()
                    .pipe(|file_states| files_pb.wrap_iter(file_states))
                    .map(|file_state| match file_state {
                        FileState::BA2File {
                            dir_hash,
                            extension,
                            index,
                            name_hash,
                            path,
                            ..
                        } => source_path
                            .join(path.into_path())
                            .pipe(|path| {
                                std::fs::OpenOptions::new()
                                    .read(true)
                                    .open(&path)
                                    .with_context(|| format!("opening [{path:?}]"))
                            })
                            .and_then(|file| {
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
                        let insert_into_archive_pb = vertical_progress_bar(entries.len() as _, ProgressKind::WriteBSA, indicatif::ProgressFinish::AndLeave)
                            .attach_to(&PROGRESS_BAR)
                            .tap_mut(|pb| {
                                pb.set_message(format!("inserting [{}] entries into archive", entries.len()));
                                pb.tick();
                            });
                        std::fs::OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .create(true)
                            .open(output_directory.clone().join(to.into_path()))
                            .context("opening output file")
                            .and_then(|mut output| {
                                entries.pipe_ref(|entries| {
                                    entries
                                        .iter()
                                        .pipe(|iter| insert_into_archive_pb.wrap_iter(iter))
                                        .try_fold(ba2::fo4::Archive::new(), |mut acc, (key, file)| match file {
                                            PreparedEntry::Normal(file) => file.as_archive_file().map(|file| {
                                                acc.insert(key.clone(), file);
                                                acc
                                            }),
                                            PreparedEntry::DX10(ba2_dx10_file) => ba2_dx10_file
                                                .borrow_mut()
                                                .take()
                                                .context("come on")
                                                .map(|file| {
                                                    acc.insert(key.clone(), file.0);
                                                    acc
                                                }),
                                        })
                                        .and_then(|archive| {
                                            let mut writer = pb.wrap_write(&mut output);
                                            archive
                                                .write(&mut writer, &Default::default())
                                                .context("writing the built archive")
                                                .and_then(|_| {
                                                    output.rewind().context("rewinding file").and_then(|_| {
                                                        let wrote = writer.progress.length().unwrap_or(0);
                                                        debug!(%wrote, "finished dumping bethesda archive");
                                                        output.flush().context("flushing").map(|_| output)
                                                    })
                                                })
                                        })
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

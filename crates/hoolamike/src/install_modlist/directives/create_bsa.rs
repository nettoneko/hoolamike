use {
    super::*,
    crate::modlist_json::{directive::CreateBSADirective, BA2DX10EntryChunk, DirectiveState, FileState},
    ba2::{CompressableFrom, ReaderWithOptions},
    std::{
        any::Any,
        convert::identity,
        io::{Seek, Write},
    },
};

#[derive(Clone, Debug)]
pub struct CreateBSAHandler {
    pub output_directory: PathBuf,
}

#[derive(derivative::Derivative, Copy)]
#[derivative(Clone(bound = ""))]
struct Leaked<T: 'static>(&'static T);

impl<T: 'static> Leaked<T> {
    pub fn new(item: T) -> Self {
        Self(Box::leak(Box::new(item)))
    }
    pub fn inner(&self) -> &'static T {
        self.0
    }
    pub unsafe fn unleak(self) {
        // SAFETY: you must manually make sure that contents are no longer referred to
        #[allow(mutable_transmutes)]
        drop(Box::from_raw(std::mem::transmute::<&'static T, &'static mut T>(self.0)))
    }
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
        tokio::task::spawn_blocking(move || {
            match state {
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
                            }
                            | FileState::BA2DX10Entry {
                                dir_hash,
                                extension,
                                index,
                                name_hash,
                                path,
                                ..
                            } => output_directory
                                .clone()
                                .join("TEMP_BSA_FILES")
                                .join(&temp_id)
                                .join(path.into_path())
                                .pipe(|path| {
                                    std::fs::OpenOptions::new()
                                        .read(true)
                                        .open(&path)
                                        .with_context(|| format!("opening [{path:?}]"))
                                })
                                .and_then(|file| {
                                    LazyArchiveFile::new(&file, false)
                                        //
                                        .and_then(|file| {
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
                                                .map(|key| (key, file))
                                        })
                                }),
                            // FileState::BA2DX10Entry {
                            //     dir_hash,
                            //     chunk_hdr_len,
                            //     chunks,
                            //     num_mips,
                            //     pixel_format,
                            //     tile_mode,
                            //     unk_8,
                            //     extension,
                            //     height,
                            //     width,
                            //     is_cube_map,
                            //     index,
                            //     name_hash,
                            //     path,
                            // } => ,
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
                                            .try_fold(ba2::fo4::Archive::new(), |mut acc, (key, file)| {
                                                file.as_archive_file().map(|file| {
                                                    acc.insert(key.clone(), file);
                                                    acc
                                                })
                                            })
                                            .and_then(|archive| {
                                                let mut writer = pb.wrap_write(&mut output);
                                                archive
                                                    .write(&mut writer, &Default::default())
                                                    .context("writing the built archive")
                                                    .and_then(|_| {
                                                        output.rewind().context("rewinding file").and_then(|_| {
                                                            let wrote = writer.progress.length().unwrap_or(0);
                                                            tracing::debug!(%wrote, "finished dumping bethesda archive");
                                                            output.flush().context("flushing").map(|_| output)
                                                        })
                                                    })
                                            })
                                    })
                                })
                        })
                }
            }
        })
        .instrument(tracing::debug_span!("performing_task"))
        .await
        .context("thread crashed")
        .and_then(identity)
        .map(|_| size)
    }
}

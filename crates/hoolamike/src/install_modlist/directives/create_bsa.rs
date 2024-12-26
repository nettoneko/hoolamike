use {
    super::*,
    crate::{
        modlist_json::{directive::CreateBSADirective, BA2DX10Entry, DirectiveState, FileState},
        progress_bars_v2::{count_progress_style, IndicatifWrapIoExt},
        utils::PathReadWrite,
    },
    ba2::fo4::Format,
    rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
    std::{
        convert::identity,
        io::{Seek, Write},
    },
    tracing::debug,
};

#[derive(Clone, Debug)]
pub struct CreateBSAHandler {
    pub output_directory: PathBuf,
}

pub mod fallout_4;

#[allow(unused_variables)]
fn try_optimize_memory_mapping(memmap: &memmap2::Mmap) {
    #[cfg(unix)]
    if let Err(err) = memmap.advise(memmap2::Advice::Sequential) {
        tracing::warn!(?err, "sequential memory mapping is not supported");
    } else {
        tracing::debug!("memory mapping optimized for sequential access")
    }
}

impl CreateBSAHandler {
    #[tracing::instrument(skip(file_states), fields(file_states_count=%file_states.len()), level = "INFO")]
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
        tokio::task::yield_now().await;
        let Self { output_directory } = self;
        tokio::task::spawn_blocking(move || {
            let source_path = output_directory
                .clone()
                .join("TEMP_BSA_FILES")
                .join(&temp_id);

            let reading_bsa_entries = info_span!("creating_bsa_entries", count=%file_states.len())
                .entered()
                .tap(|pb| {
                    pb.pb_set_style(&count_progress_style());
                    pb.pb_set_length(file_states.len() as _);
                });
            match state {
                DirectiveState::CompressionBsa {
                    archive_flags,
                    file_flags,
                    magic,
                    version,
                } => Err(anyhow::anyhow!("not implemented")),
                DirectiveState::CompressionBa2 {
                    has_name_table: _,
                    header_magic: _,
                    kind: _,
                    version: _,
                } => {
                    file_states
                        .into_par_iter()
                        .map(move |file_state| match file_state {
                            FileState::BSAFile { .. } => Err(anyhow::anyhow!("mismatched type of file")),
                            FileState::BA2File {
                                dir_hash,
                                extension,
                                name_hash,
                                path,
                                ..
                            } => source_path
                                .join(path.into_path())
                                .pipe(|path| path.open_file_read())
                                .and_then(|(_path, file)| fallout_4::LazyArchiveFile::new(&file, false))
                                .and_then(|file| fallout_4::create_key(&extension, name_hash, dir_hash).map(|key| (key, file))),
                            FileState::BA2DX10Entry(ba2_dx10_entry) => fallout_4::LazyArchiveFile::new_dx_entry(source_path.clone(), ba2_dx10_entry.clone())
                                .and_then(|file| {
                                    ba2_dx10_entry.pipe(
                                        |BA2DX10Entry {
                                             dir_hash,
                                             extension,
                                             name_hash,
                                             ..
                                         }| {
                                            fallout_4::create_key(&extension, name_hash, dir_hash).map(|key| (key, file))
                                        },
                                    )
                                }),
                        })
                        .inspect(|_| reading_bsa_entries.pb_inc(1))
                        .collect::<Result<Vec<_>>>()
                }
                .and_then(|entries| {
                    let building_archive = info_span!("building_archive").entered().tap(|pb| {
                        pb.pb_set_style(&count_progress_style());
                        pb.pb_set_length(entries.len() as _);
                    });
                    output_directory
                        .clone()
                        .join(to.into_path())
                        .open_file_write()
                        .and_then(|(output_path, mut output)| {
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
                                                ba2::fo4::FileHeader::GNRL => Format::GNRL,
                                                ba2::fo4::FileHeader::DX10(_) => Format::DX10,
                                                ba2::fo4::FileHeader::GNMF(_) => Format::GNMF,
                                            })
                                            .unwrap_or_default()
                                            .pipe(|format| ba2::fo4::ArchiveOptions::builder().format(format))
                                            .pipe(|options| {
                                                entries
                                                    .into_iter()
                                                    .fold(ba2::fo4::Archive::new(), |acc, (key, file)| {
                                                        acc.tap_mut(|acc| {
                                                            acc.insert(key.clone(), file);
                                                        })
                                                    })
                                                    .pipe(|archive| {
                                                        {
                                                            let mut writer = tracing::Span::current().wrap_write(size, &mut output);
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
                                            })
                                    })
                                    .with_context(|| format!("writing to [{:?}]", output_path))
                            })
                        })
                }),
            }
        })
        .instrument(tracing::Span::current())
        .await
        .context("thread crashed")
        .and_then(identity)
        .map(|_| size)
    }
}

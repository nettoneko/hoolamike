use {
    super::*,
    crate::{
        compression::{forward_only_seek::ForwardOnlySeek, ProcessArchive},
        install_modlist::download_cache::{to_u64_from_base_64, validate_hash},
        modlist_json::directive::PatchedFromArchiveDirective,
        progress_bars::{vertical_progress_bar, ProgressKind, PROGRESS_BAR},
        read_wrappers::ReadExt,
    },
    indicatif::ProgressBar,
    std::{
        convert::identity,
        io::{Read, Seek, Write},
    },
};

#[derive(Clone, Debug)]
pub struct PatchedFromArchiveHandler {
    pub nested_archive_service: Arc<Mutex<NestedArchivesService>>,
    pub wabbajack_file: WabbajackFileHandle,
    pub output_directory: PathBuf,
}

impl PatchedFromArchiveHandler {
    #[tracing::instrument]
    pub async fn handle(
        self,
        PatchedFromArchiveDirective {
            hash,
            size,
            to,
            archive_hash_path,
            from_hash: _,
            patch_id,
        }: PatchedFromArchiveDirective,
    ) -> Result<()> {
        let output_path = self.output_directory.join(to.into_path());

        if let Err(message) = validate_hash(output_path.clone(), hash.clone()).await {
            let source_file = self
                .nested_archive_service
                .lock()
                .await
                .get(archive_hash_path.clone())
                .await
                .context("could not get a handle to archive")?;

            tokio::task::spawn_blocking(move || -> Result<_> {
                let pb = vertical_progress_bar(size, ProgressKind::Extract, indicatif::ProgressFinish::AndClear)
                    .attach_to(&PROGRESS_BAR)
                    .tap_mut(|pb| {
                        pb.set_message(output_path.display().to_string());
                    });

                let mut wabbajack_file = self.wabbajack_file.blocking_lock();

                fn perform_copy<S, D, T>(pb: ProgressBar, source: S, delta: D, target: T, expected_size: u64, expected_hash: String) -> Result<()>
                where
                    S: Read + Seek,
                    D: Read,
                    T: Write,
                {
                    // this applies delta on the fly
                    let from = crate::octadiff_reader::ApplyDetla::new_from_readers(source, ForwardOnlySeek::new(delta))
                        .context("invalid delta")?
                        .context("delta is empty")?;
                    let mut writer = &mut std::io::BufWriter::new(target);
                    std::io::copy(
                        &mut pb
                            .wrap_read(from)
                            .and_validate_size(expected_size)
                            .and_validate_hash(to_u64_from_base_64(expected_hash)?),
                        &mut writer,
                    )
                    .context("copying file from archive")
                    .and_then(|_| writer.flush().context("flushing"))
                    .map(|_| ())
                }
                let delta_file = wabbajack_file
                    .get_handle(Path::new(&patch_id.hyphenated().to_string()))
                    .with_context(|| format!("patch {patch_id:?} does not exist"))?;

                match source_file {
                    nested_archive_manager::HandleKind::Cached(file) => file
                        .inner
                        .1
                        .try_clone()
                        .context("cloning file")
                        .and_then(|mut f| f.rewind().context("rewinding").map(|_| f)),
                    nested_archive_manager::HandleKind::JustHashPath(source_file_path) => std::fs::OpenOptions::new()
                        .read(true)
                        .open(&source_file_path)
                        .with_context(|| format!("opening [{}]", source_file_path.display())),
                }
                .and_then(|mut final_source| {
                    create_file_all(&output_path).and_then(|mut output_file| {
                        perform_copy(pb, &mut final_source, delta_file, &mut output_file, size, hash)
                            .with_context(|| format!("when extracting from [{:?}] to [{}]", archive_hash_path, output_path.display()))
                    })
                })
            })
            .await
            .context("thread crashed")
            .and_then(identity)?;
        }
        Ok(())
    }
}

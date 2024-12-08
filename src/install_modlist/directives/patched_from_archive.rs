use {
    super::*,
    crate::{
        compression::{forward_only_seek::ForwardOnlySeek, ProcessArchive},
        install_modlist::download_cache::validate_hash,
        modlist_json::directive::{ArchiveHashPath, PatchedFromArchiveDirective},
        progress_bars::{print_error, vertical_progress_bar, ProgressKind, PROGRESS_BAR},
    },
    indicatif::ProgressBar,
    std::{
        convert::identity,
        io::{Read, Seek, Write},
    },
};

#[derive(Clone, Debug)]
pub struct PatchedFromArchiveHandler {
    pub wabbajack_file: WabbajackFileHandle,
    pub output_directory: PathBuf,
    pub download_summary: Arc<BTreeMap<String, WithArchiveDescriptor<PathBuf>>>,
}

impl PatchedFromArchiveHandler {
    pub async fn handle(
        self,
        PatchedFromArchiveDirective {
            hash,
            size,
            to,
            archive_hash_path,
            from_hash,
            patch_id,
        }: PatchedFromArchiveDirective,
    ) -> Result<()> {
        let output_path = self.output_directory.join(to.into_path());

        if let Err(message) = validate_hash(output_path.clone(), hash).await {
            print_error(output_path.display().to_string(), &message);
            tokio::task::spawn_blocking(move || -> Result<_> {
                let pb = vertical_progress_bar(size, ProgressKind::Extract, indicatif::ProgressFinish::AndClear)
                    .attach_to(&PROGRESS_BAR)
                    .tap_mut(|pb| {
                        pb.set_message(output_path.display().to_string());
                    });

                let mut wabbajack_file = self.wabbajack_file.blocking_lock();
                let delta_file = wabbajack_file
                    .get_handle(Path::new(&patch_id.hyphenated().to_string()))
                    .with_context(|| format!("patch {patch_id:?} does not exist"))?;

                fn perform_copy<S, D, T>(pb: ProgressBar, source: S, delta: D, target: T) -> Result<()>
                where
                    S: Read,
                    D: Read,
                    T: Write,
                {
                    // this applies delta on the fly
                    let from = crate::octadiff_reader::ApplyDetla::new_from_readers(ForwardOnlySeek::new(source), ForwardOnlySeek::new(delta))
                        .context("invalid delta")?
                        .context("delta is empty")?;
                    let mut writer = &mut std::io::BufWriter::new(target);
                    std::io::copy(&mut pb.wrap_read(from), &mut writer)
                        .context("copying file from archive")
                        .and_then(|_| writer.flush().context("flushing"))
                        .map(|_| ())
                }

                match archive_hash_path {
                    ArchiveHashPath::ArchiveHashAndPath((source_hash, source_path)) => {
                        let source_path = source_path.into_path();
                        let source = self
                            .download_summary
                            .get(&source_hash)
                            .with_context(|| format!("directive expected hash [{source_hash}], but no such item was produced"))?;
                        debug!(?source, "found source_file");
                        let source_file = source.inner.clone();

                        let mut output_file = create_file_all(&output_path)?;
                        let mut archive = std::fs::OpenOptions::new()
                            .read(true)
                            .open(&source_file)
                            .with_context(|| format!("opening [{}]", source_file.display()))
                            .and_then(|file| {
                                crate::compression::ArchiveHandle::guess(file)
                                    .map_err(|_file| anyhow::anyhow!("no compression algorithm matched file [{}]", source_file.display()))
                            })?;
                        archive
                            .get_handle(Path::new(&source_path))
                            .and_then(|mut source_file| perform_copy(pb, &mut source_file, delta_file, &mut output_file))
                            .map(|_| ())
                    }
                    ArchiveHashPath::JustArchiveHash((source_hash,)) => {
                        let source = self
                            .download_summary
                            .get(&source_hash)
                            .with_context(|| format!("directive expected hash [{source_hash}], but no such item was produced"))?;
                        debug!(?source, "found source_file");
                        let mut source_file = std::fs::OpenOptions::new()
                            .read(true)
                            .open(&source.inner)
                            .with_context(|| format!("when opening [{}]", source.inner.display()))?;
                        let mut output_file = create_file_all(&output_path)?;
                        perform_copy(pb, &mut source_file, delta_file, &mut output_file)
                    }
                    other => anyhow::bail!("not implemented: {other:#?}"),
                }
            })
            .await
            .context("thread crashed")
            .and_then(identity)?;
        }
        Ok(())
    }
}

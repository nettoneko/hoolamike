use {
    super::*,
    crate::{
        compression::ProcessArchive,
        install_modlist::download_cache::validate_hash,
        modlist_json::directive::{ArchiveHashPath, FromArchiveDirective},
        progress_bars::{print_error, vertical_progress_bar, ProgressKind, PROGRESS_BAR},
    },
    std::{
        convert::identity,
        io::{Read, Write},
        path::Path,
    },
};

#[derive(Clone, Debug)]
pub struct FromArchiveHandler {
    pub download_summary: DownloadSummary,
    pub output_directory: PathBuf,
}

impl FromArchiveHandler {
    pub async fn handle(
        self,
        FromArchiveDirective {
            hash,
            size,
            to,
            archive_hash_path,
        }: FromArchiveDirective,
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
                let perform_copy = move |from: &mut dyn Read, to: &mut dyn Write| {
                    let mut writer = std::io::BufWriter::new(to);
                    std::io::copy(&mut pb.wrap_read(from), &mut writer)
                        .context("copying file from archive")
                        .and_then(|_| writer.flush().context("flushing write"))
                        .map(|_| ())
                };

                match archive_hash_path {
                    ArchiveHashPath::ArchiveHashAndPath((source_hash, source_path)) => {
                        let source_path = source_path.into_path();
                        let source = self
                            .download_summary
                            .get(&source_hash)
                            .with_context(|| format!("directive expected hash [{source_hash}], but no such item was produced"))?;
                        info!(?source, "found source");
                        let source_file_path = source.inner.clone();

                        let mut output_file = create_file_all(&output_path)?;
                        let mut archive = std::fs::OpenOptions::new()
                            .read(true)
                            .open(&source_file_path)
                            .with_context(|| format!("opening [{}]", source_file_path.display()))
                            .and_then(|source_file| {
                                crate::compression::ArchiveHandle::guess(source_file, &source_file_path)
                                    .map_err(|_file| anyhow::anyhow!("no compression algorithm matched file [{}]", source_file_path.display()))
                            })?;
                        archive
                            .get_handle(Path::new(&source_path))
                            .and_then(|mut file| perform_copy(&mut file, &mut output_file))
                            .map(|_| ())
                    }
                    ArchiveHashPath::JustArchiveHash((source_hash,)) => {
                        let source = self
                            .download_summary
                            .get(&source_hash)
                            .with_context(|| format!("directive expected hash [{source_hash}], but no such item was produced"))?;
                        info!(?source, "found source");
                        let mut source_file = std::fs::OpenOptions::new()
                            .read(true)
                            .open(&source.inner)
                            .with_context(|| format!("when opening [{}]", source.inner.display()))?;
                        let mut output_file = create_file_all(&output_path)?;
                        perform_copy(&mut source_file, &mut output_file)
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

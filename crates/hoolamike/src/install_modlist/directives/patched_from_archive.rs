use {
    super::*,
    crate::{
        compression::{forward_only_seek::ForwardOnlySeek, ProcessArchive},
        install_modlist::download_cache::{to_u64_from_base_64, validate_hash},
        modlist_json::directive::PatchedFromArchiveDirective,
        progress_bars_v2::IndicatifWrapIoExt,
        read_wrappers::ReadExt,
    },
    std::{
        convert::identity,
        io::{Read, Seek, Write},
    },
    tracing::Instrument,
};

#[derive(Clone, derivative::Derivative)]
#[derivative(Debug)]
pub struct PatchedFromArchiveHandler {
    #[derivative(Debug = "ignore")]
    pub nested_archive_service: Arc<NestedArchivesService>,
    pub wabbajack_file: WabbajackFileHandle,
    pub output_directory: PathBuf,
}

impl PatchedFromArchiveHandler {
    #[tracing::instrument(skip(self), level = "INFO")]
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
    ) -> Result<u64> {
        let output_path = self.output_directory.join(to.into_path());

        if let Err(message) = validate_hash(output_path.clone(), hash.clone()).await {
            tracing::warn!(?message);
            let source_file = self
                .nested_archive_service
                .clone()
                .get(archive_hash_path.clone())
                .await
                .context("could not get a handle to archive")?;

            tokio::task::spawn_blocking(move || -> Result<_> {
                let mut wabbajack_file = self.wabbajack_file.blocking_lock();
                #[tracing::instrument(skip(source, delta, target), level = "INFO")]
                fn perform_copy<S, D, T>(source: S, delta: D, target: T, expected_size: u64, expected_hash: String) -> Result<()>
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
                        &mut tracing::Span::current()
                            .wrap_read(expected_size, from)
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

                source_file
                    .open_file_read()
                    .and_then(|(final_source_path, mut final_source)| {
                        create_file_all(&output_path).and_then(|mut output_file| {
                            perform_copy(&mut final_source, delta_file, &mut output_file, size, hash)
                                .with_context(|| format!("when extracting from [{final_source_path:?}] to [{output_path:?}]"))
                                .with_context(|| format!("when handling [{archive_hash_path:?}] copy"))
                        })
                    })
            })
            .instrument(tracing::Span::current())
            .await
            .context("thread crashed")
            .and_then(identity)?;
        }
        Ok(size)
    }
}

use {
    super::*,
    crate::{
        compression::ProcessArchive,
        install_modlist::download_cache::validate_hash,
        modlist_json::directive::InlineFileDirective,
        progress_bars_v2::IndicatifWrapIoExt,
    },
    std::{convert::identity, io::Write, path::Path},
};

#[derive(Clone, Debug)]
pub struct InlineFileHandler {
    pub wabbajack_file: WabbajackFileHandle,
    pub output_directory: PathBuf,
}

impl InlineFileHandler {
    #[tracing::instrument]
    pub async fn handle(
        self,
        InlineFileDirective {
            hash,
            size,
            source_data_id,
            to,
        }: InlineFileDirective,
    ) -> Result<u64> {
        let output_path = self.output_directory.join(to.into_path());
        if let Err(message) = validate_hash(output_path.clone(), hash.clone()).await {
            tracing::warn!(?message);

            let wabbajack_file = self.wabbajack_file.clone();
            tokio::task::spawn_blocking(move || -> Result<_> {
                let output_file = create_file_all(&output_path)?;

                let mut archive = wabbajack_file.blocking_lock();
                archive
                    .get_handle(Path::new(&source_data_id.as_hyphenated().to_string()))
                    .and_then(|file| {
                        let mut writer = std::io::BufWriter::new(output_file);
                        std::io::copy(
                            &mut tracing::Span::current().wrap_read(size, file),
                            // WARN: stuff that's inside modlist.wabbajack/modlist(.json) is incorrect
                            // .and_validate_size(size)
                            // .and_validate_hash(hash.pipe(to_u64_from_base_64).expect("come on")),
                            &mut writer,
                        )
                        .context("copying file from archive")
                        .and_then(|_| writer.flush().context("flushing"))
                    })
                    .map(|_| ())
            })
            .await
            .context("thread crashed")
            .and_then(identity)?;
        }
        Ok(size)
    }
}

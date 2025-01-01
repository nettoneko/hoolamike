use {
    super::*,
    crate::{modlist_json::directive::InlineFileDirective, progress_bars_v2::IndicatifWrapIoExt, utils::spawn_rayon},
    std::{io::Write, path::Path},
};

#[derive(Clone, Debug)]
pub struct InlineFileHandler {
    pub wabbajack_file: WabbajackFileHandle,
    pub output_directory: PathBuf,
}

impl InlineFileHandler {
    #[tracing::instrument(skip(self))]
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
        let wabbajack_file = self.wabbajack_file.clone();
        spawn_rayon(move || -> Result<_> {
            let output_file = create_file_all(&output_path)?;

            let archive = wabbajack_file;
            archive
                .get_archive()
                .and_then(|mut archive| archive.get_handle(Path::new(&source_data_id.as_hyphenated().to_string())))
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
        .map(|_| size)
    }
}

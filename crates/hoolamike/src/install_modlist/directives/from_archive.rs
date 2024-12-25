use {
    super::*,
    crate::{
        install_modlist::download_cache::{to_u64_from_base_64, validate_file_size, validate_hash},
        modlist_json::directive::FromArchiveDirective,
        progress_bars_v2::IndicatifWrapIoExt,
        read_wrappers::ReadExt,
    },
    std::{
        convert::identity,
        io::{Read, Write},
        path::Path,
    },
    tracing::{info_span, Instrument},
};

#[derive(Clone, derivative::Derivative)]
#[derivative(Debug)]
pub struct FromArchiveHandler {
    #[derivative(Debug = "ignore")]
    pub nested_archive_service: Arc<NestedArchivesService>,
    pub output_directory: PathBuf,
}

const EXTENSION_HASH_WHITELIST: &[&str] = &[
    // hashes won't match because headers are also hashed in wabbajack
    "dds",
];

fn is_whitelisted_by_path(path: &Path) -> bool {
    matches!(
        path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .as_deref(),
        Some(ext) if EXTENSION_HASH_WHITELIST.contains(&ext)
    )
}

pub async fn validate_hash_with_overrides(path: PathBuf, hash: String, size: u64) -> Result<PathBuf> {
    match is_whitelisted_by_path(&path) {
        true => validate_file_size(path, size).await,
        false => validate_hash(path, hash).await,
    }
}

impl FromArchiveHandler {
    #[tracing::instrument(skip(self), level = "INFO")]
    pub async fn handle(
        self,
        FromArchiveDirective {
            hash,
            size,
            to,
            archive_hash_path,
        }: FromArchiveDirective,
    ) -> Result<u64> {
        let output_path = self.output_directory.join(to.into_path());

        let source_file = self
            .nested_archive_service
            .clone()
            .get(archive_hash_path.clone())
            .instrument(info_span!("obtaining_nested_archive", ?archive_hash_path))
            .await
            .context("could not get a handle to archive")?;

        tokio::task::spawn_blocking(move || -> Result<_> {
            let perform_copy = move |from: &mut dyn Read, to: &mut dyn Write, target_path: PathBuf| {
                info_span!("perform_copy").in_scope(|| {
                    let mut writer = to;
                    let mut reader: Box<dyn Read> = match is_whitelisted_by_path(&target_path) {
                        true => tracing::Span::current()
                            // WARN: hashes are not gonna match for bsa stuff because we write headers differentlys
                            .wrap_read(size, from)
                            .and_validate_size(size)
                            .pipe(Box::new),
                        false => tracing::Span::current()
                            .wrap_read(size, from)
                            .and_validate_size(size)
                            .and_validate_hash(hash.pipe(to_u64_from_base_64).expect("come on"))
                            .pipe(Box::new),
                    };
                    std::io::copy(&mut reader, &mut writer)
                        .context("copying file from archive")
                        .and_then(|_| writer.flush().context("flushing write"))
                        .map(|_| ())
                })
            };

            source_file
                .open_file_read()
                .and_then(|(source_path, mut final_source)| {
                    create_file_all(&output_path).and_then(|mut output_file| {
                        perform_copy(&mut final_source, &mut output_file, output_path.clone()).with_context(|| {
                            format!(
                                "when extracting from [{source_path:?}] ({:?}) to [{}]",
                                archive_hash_path,
                                output_path.display()
                            )
                        })
                    })
                })?;
            Ok(())
        })
        .instrument(tracing::Span::current())
        .await
        .context("thread crashed")
        .and_then(identity)?;
        Ok(size)
    }
}

use {
    super::*,
    crate::{install_modlist::download_cache::to_u64_from_base_64, modlist_json::directive::TransformedTextureDirective, read_wrappers::ReadExt},
    std::{
        convert::identity,
        io::{Read, Seek, Write},
    },
};

#[derive(Clone, Debug)]
pub struct TransformedTextureHandler {
    pub output_directory: PathBuf,
    pub nested_archive_service: Arc<Mutex<NestedArchivesService>>,
}

#[extension_traits::extension(pub trait IoResultValidateSizeExt)]
impl std::io::Result<u64> {
    fn and_validate_size(self, expected_size: u64) -> anyhow::Result<u64> {
        self.context("performing read").and_then(|size| {
            size.eq(&expected_size)
                .then_some(size)
                .with_context(|| format!("expected [{expected_size} bytes], but [{size} bytes] was read"))
        })
    }
}

// #[cfg(feature = "dds_recompression")]
mod dds_recompression;

impl TransformedTextureHandler {
    pub async fn handle(
        self,
        TransformedTextureDirective {
            hash,
            size,
            image_state,
            to,
            archive_hash_path,
        }: TransformedTextureDirective,
    ) -> Result<u64> {
        let output_path = self.output_directory.join(to.into_path());

        if let Err(message) = validate_hash_with_overrides(output_path.clone(), hash.clone(), size).await {
            let source_file = self
                .nested_archive_service
                .lock()
                .instrument(info_span!("obtaining_archive_service_lock"))
                .await
                .get(archive_hash_path.clone())
                .instrument(info_span!("obtaining_nested_archive", ?archive_hash_path))
                .await
                .context("could not get a handle to archive")?;

            tokio::task::spawn_blocking(move || -> Result<_> {
                let pb = vertical_progress_bar(size, ProgressKind::Extract, indicatif::ProgressFinish::AndClear)
                    .attach_to(&PROGRESS_BAR)
                    .tap_mut(|pb| {
                        pb.set_message(output_path.display().to_string());
                    });
                let perform_copy = move |from: &mut dyn Read, to: &mut dyn Write, target_path: PathBuf| {
                    info_span!("perform_copy").in_scope(|| {
                        let mut writer = to;
                        let mut reader: Box<dyn Read> = match is_whitelisted_by_path(&target_path) {
                            true => pb.wrap_read(from).pipe(Box::new),
                            false => pb
                                .wrap_read(from)
                                .and_validate_hash(hash.pipe(to_u64_from_base_64).expect("come on"))
                                .pipe(Box::new),
                        };
                        std::io::copy(
                            &mut reader,
                            // WARN: hashes are not gonna match for bsa stuff because we write headers differentlys
                            // .and_validate_hash(hash.pipe(to_u64_from_base_64).expect("come on")),
                            &mut writer,
                        )
                        // .and_validate_size(size)
                        .context("copying file from archive")
                        .and_then(|_| writer.flush().context("flushing write"))
                        .map(|_| ())
                    })
                };

                match source_file {
                    nested_archive_manager::HandleKind::Cached(file) => file
                        .inner
                        .blocking_lock()
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
                        perform_copy(&mut final_source, &mut output_file, output_path.clone())
                            .with_context(|| format!("when extracting from [{:?}] to [{}]", archive_hash_path, output_path.display()))
                    })
                })?;
                Ok(())
            })
            .instrument(tracing::Span::current())
            .await
            .context("thread crashed")
            .and_then(identity)?;
        }
        Ok(size)
    }
}

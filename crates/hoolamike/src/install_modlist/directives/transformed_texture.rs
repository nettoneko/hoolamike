use {
    super::*,
    crate::{
        downloaders::helpers::FutureAnyhowExt,
        install_modlist::download_cache::to_u64_from_base_64,
        modlist_json::{directive::TransformedTextureDirective, ImageState},
        progress_bars_v2::IndicatifWrapIoExt,
        read_wrappers::ReadExt,
        utils::spawn_rayon,
    },
    queued_archive_task::QueuedArchiveService,
    std::io::{Read, Write},
};

#[derive(Clone, derivative::Derivative)]
#[derivative(Debug)]
pub struct TransformedTextureHandler {
    pub output_directory: PathBuf,
    #[derivative(Debug = "ignore")]
    pub archive_extraction_queue: Arc<QueuedArchiveService>,
    #[derivative(Debug = "ignore")]
    pub download_summary: DownloadSummary,
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
mod dds_recompression_v2;

impl TransformedTextureHandler {
    #[instrument(skip(self))]
    pub async fn handle(
        self,
        TransformedTextureDirective {
            hash,
            size,
            image_state:
                ImageState {
                    format,
                    height,
                    mip_levels,
                    perceptual_hash: _,
                    width,
                },
            to,
            archive_hash_path,
        }: TransformedTextureDirective,
    ) -> Result<u64> {
        let handle = tracing::Span::current();
        // let _image_dds_format = supported_image_format(format).context("checking for format support")?;
        let output_path = self.output_directory.join(to.into_path());
        let source_file = self
            .download_summary
            .resolve_archive_path(&archive_hash_path)
            .pipe(ready)
            .and_then(|path| {
                self.archive_extraction_queue
                    .get_archive(path)
                    .map_context("awaiting for archive from queue")
            })
            .await
            .with_context(|| format!("reading archive for [{archive_hash_path:?}]"))?;

        spawn_rayon(move || -> Result<_> {
            handle.in_scope(|| {
                let perform_copy = {
                    move |from: &mut dyn Read, to: &mut dyn Write, target_path: PathBuf| {
                        info_span!("perform_copy").in_scope(|| {
                            let mut writer = to;
                            let mut reader: Box<dyn Read> = match is_whitelisted_by_path(&target_path) {
                                true => tracing::Span::current()
                                    .wrap_read(size, from)
                                    .pipe(Box::new),
                                false => tracing::Span::current()
                                    .wrap_read(size, from)
                                    .and_validate_hash(hash.pipe(to_u64_from_base_64).expect("come on"))
                                    .pipe(Box::new),
                            };
                            dds_recompression_v2::resize_dds(&mut reader, width, height, format, mip_levels, &mut writer)
                                .context("copying file from archive")
                                .and_then(|_| writer.flush().context("flushing write"))
                                .map(|_| ())
                        })
                    }
                };

                source_file
                    .open_file_read()
                    .and_then(|(source_path, mut final_source)| {
                        create_file_all(&output_path).and_then(|mut output_file| {
                            perform_copy(&mut final_source, &mut output_file, output_path.clone())
                                // .or_else(|reason| {
                                //     let _span =
                                //         tracing::error_span!("could not resize texture, copying the original", reason = %format!("{reason:?}")).entered();
                                //     tracing::error!("could not resize the file, but it should still work");
                                //     final_source
                                //         .rewind()
                                //         .context("rewinding original file")
                                //         .map(|_| final_source)
                                //         .and_then(|final_source| {
                                //             output_path.open_file_write().and_then(|(_, mut output)| {
                                //                 std::io::copy(&mut tracing::Span::current().wrap_read(size, final_source), &mut output)
                                //                     .with_context(|| format!("writing original because resizing could not be performed due to: {reason:?}"))
                                //             })
                                //         })
                                //         .map(|_| ())
                                // })
                                .with_context(|| format!("when extracting from [{source_path:?}]({:?}) to [{}]", archive_hash_path, output_path.display()))
                        })
                    })?;
                Ok(())
            })
        })
        .instrument(tracing::Span::current())
        .await
        .map(|_| size)
    }
}

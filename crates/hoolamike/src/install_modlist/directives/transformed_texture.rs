use {
    super::*,
    crate::{
        install_modlist::download_cache::to_u64_from_base_64,
        modlist_json::{directive::TransformedTextureDirective, ImageState},
        progress_bars_v2::IndicatifWrapIoExt,
        read_wrappers::ReadExt,
    },
    std::{
        convert::identity,
        io::{Read, Seek, Write},
    },
};

#[derive(Clone, derivative::Derivative)]
#[derivative(Debug)]
pub struct TransformedTextureHandler {
    pub output_directory: PathBuf,
    #[derivative(Debug = "ignore")]
    pub nested_archive_service: Arc<NestedArchivesService>,
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

fn supported_image_format(format: crate::modlist_json::image_format::DXGIFormat) -> Result<image_dds::ImageFormat> {
    use crate::modlist_json::image_format::DXGIFormat;
    // TODO: validate this
    match format {
        DXGIFormat::R8_UNORM => image_dds::ImageFormat::R8Unorm,
        DXGIFormat::R8G8B8A8_UNORM => image_dds::ImageFormat::Rgba8Unorm,
        DXGIFormat::R8G8B8A8_UNORM_SRGB => image_dds::ImageFormat::Rgba8UnormSrgb,
        DXGIFormat::R16G16B16A16_FLOAT => image_dds::ImageFormat::Rgba16Float,
        DXGIFormat::R32G32B32A32_FLOAT => image_dds::ImageFormat::Rgba32Float,
        DXGIFormat::B8G8R8A8_UNORM => image_dds::ImageFormat::Bgra8Unorm,
        DXGIFormat::B8G8R8A8_UNORM_SRGB => image_dds::ImageFormat::Bgra8UnormSrgb,
        DXGIFormat::B4G4R4A4_UNORM => image_dds::ImageFormat::Bgra4Unorm,
        DXGIFormat::BC1_UNORM => image_dds::ImageFormat::BC1RgbaUnorm,
        DXGIFormat::BC1_UNORM_SRGB => image_dds::ImageFormat::BC1RgbaUnormSrgb,
        DXGIFormat::BC2_UNORM => image_dds::ImageFormat::BC2RgbaUnorm,
        DXGIFormat::BC2_UNORM_SRGB => image_dds::ImageFormat::BC2RgbaUnormSrgb,
        DXGIFormat::BC3_UNORM => image_dds::ImageFormat::BC3RgbaUnorm,
        DXGIFormat::BC3_UNORM_SRGB => image_dds::ImageFormat::BC3RgbaUnormSrgb,
        DXGIFormat::BC4_UNORM => image_dds::ImageFormat::BC4RUnorm,
        DXGIFormat::BC4_SNORM => image_dds::ImageFormat::BC4RSnorm,
        DXGIFormat::BC5_UNORM => image_dds::ImageFormat::BC5RgUnorm,
        DXGIFormat::BC5_SNORM => image_dds::ImageFormat::BC5RgSnorm,
        DXGIFormat::BC6H_UF16 => image_dds::ImageFormat::BC6hRgbUfloat,
        DXGIFormat::BC6H_SF16 => image_dds::ImageFormat::BC6hRgbSfloat,
        DXGIFormat::BC7_UNORM => image_dds::ImageFormat::BC7RgbaUnorm,
        DXGIFormat::BC7_UNORM_SRGB => image_dds::ImageFormat::BC7RgbaUnormSrgb,
        // WARN: hacks
        unsupported => anyhow::bail!("{unsupported:?} is not supported"),
    }
    .pipe(Ok)
}

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
        let format = supported_image_format(format).context("checking for format support")?;
        let output_path = self.output_directory.join(to.into_path());
        let handle = tracing::Span::current();

        if let Err(_message) = validate_hash_with_overrides(output_path.clone(), hash.clone(), size).await {
            let source_file = self
                .nested_archive_service
                .clone()
                .get(archive_hash_path.clone())
                .instrument(info_span!("obtaining_nested_archive", ?archive_hash_path))
                .await
                .context("could not get a handle to archive")?;

            tokio::task::spawn_blocking(move || -> Result<_> {
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
                                dds_recompression::resize_dds(&mut reader, width, height, format, mip_levels, &mut writer)
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
                                    .or_else(|reason| {
                                        let _span = tracing::error_span!("could not resize texture, copying the original", ?reason).entered();
                                        tracing::error!("could not resize the file, but it should still work");
                                        final_source
                                            .rewind()
                                            .context("rewinding original file")
                                            .map(|_| final_source)
                                            .and_then(|final_source| {
                                                output_path.open_file_write().and_then(|(_, mut output)| {
                                                    std::io::copy(&mut tracing::Span::current().wrap_read(size, final_source), &mut output)
                                                        .with_context(|| format!("writing original because resizing could not be performed due to: {reason:?}"))
                                                })
                                            })
                                            .map(|_| ())
                                    })
                                    .with_context(|| format!("when extracting from [{source_path:?}]({:?}) to [{}]", archive_hash_path, output_path.display()))
                            })
                        })?;
                    Ok(())
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

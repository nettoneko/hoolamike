use {
    super::*,
    crate::{
        install_modlist::download_cache::to_u64_from_base_64,
        modlist_json::{directive::TransformedTextureDirective, ImageState},
        read_wrappers::ReadExt,
    },
    std::{
        convert::identity,
        io::{Read, Write},
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
    // TODO: validate this
    match format {
        crate::modlist_json::image_format::DXGIFormat::R8_UNORM => image_dds::ImageFormat::R8Unorm,
        crate::modlist_json::image_format::DXGIFormat::R8G8B8A8_UNORM => image_dds::ImageFormat::Rgba8Unorm,
        crate::modlist_json::image_format::DXGIFormat::R8G8B8A8_UNORM_SRGB => image_dds::ImageFormat::Rgba8UnormSrgb,
        crate::modlist_json::image_format::DXGIFormat::R16G16B16A16_FLOAT => image_dds::ImageFormat::Rgba16Float,
        crate::modlist_json::image_format::DXGIFormat::R32G32B32A32_FLOAT => image_dds::ImageFormat::Rgba32Float,
        crate::modlist_json::image_format::DXGIFormat::B8G8R8A8_UNORM => image_dds::ImageFormat::Bgra8Unorm,
        crate::modlist_json::image_format::DXGIFormat::B8G8R8A8_UNORM_SRGB => image_dds::ImageFormat::Bgra8UnormSrgb,
        crate::modlist_json::image_format::DXGIFormat::B4G4R4A4_UNORM => image_dds::ImageFormat::Bgra4Unorm,
        crate::modlist_json::image_format::DXGIFormat::BC1_UNORM => image_dds::ImageFormat::BC1RgbaUnorm,
        crate::modlist_json::image_format::DXGIFormat::BC1_UNORM_SRGB => image_dds::ImageFormat::BC1RgbaUnormSrgb,
        crate::modlist_json::image_format::DXGIFormat::BC2_UNORM => image_dds::ImageFormat::BC2RgbaUnorm,
        crate::modlist_json::image_format::DXGIFormat::BC2_UNORM_SRGB => image_dds::ImageFormat::BC2RgbaUnormSrgb,
        crate::modlist_json::image_format::DXGIFormat::BC3_UNORM => image_dds::ImageFormat::BC3RgbaUnorm,
        crate::modlist_json::image_format::DXGIFormat::BC3_UNORM_SRGB => image_dds::ImageFormat::BC3RgbaUnormSrgb,
        crate::modlist_json::image_format::DXGIFormat::BC4_UNORM => image_dds::ImageFormat::BC4RUnorm,
        crate::modlist_json::image_format::DXGIFormat::BC4_SNORM => image_dds::ImageFormat::BC4RSnorm,
        crate::modlist_json::image_format::DXGIFormat::BC5_UNORM => image_dds::ImageFormat::BC5RgUnorm,
        crate::modlist_json::image_format::DXGIFormat::BC5_SNORM => image_dds::ImageFormat::BC5RgSnorm,
        crate::modlist_json::image_format::DXGIFormat::BC6H_UF16 => image_dds::ImageFormat::BC6hRgbUfloat,
        crate::modlist_json::image_format::DXGIFormat::BC6H_SF16 => image_dds::ImageFormat::BC6hRgbSfloat,
        crate::modlist_json::image_format::DXGIFormat::BC7_UNORM => image_dds::ImageFormat::BC7RgbaUnorm,
        crate::modlist_json::image_format::DXGIFormat::BC7_UNORM_SRGB => image_dds::ImageFormat::BC7RgbaUnormSrgb,
        unsupported => anyhow::bail!("{unsupported:?} is not supported"),
    }
    .pipe(Ok)
}

impl TransformedTextureHandler {
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

        if let Err(message) = validate_hash_with_overrides(output_path.clone(), hash.clone(), size).await {
            tracing::warn!(?message);
            let source_file = self
                .nested_archive_service
                .clone()
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
                        dds_recompression::resize_dds(&mut reader, width, height, format, mip_levels, &mut writer)
                            .context("copying file from archive")
                            .and_then(|_| writer.flush().context("flushing write"))
                            .map(|_| ())
                    })
                };

                source_file
                    .open_file_read()
                    .and_then(|(source_path, mut final_source)| {
                        create_file_all(&output_path).and_then(|mut output_file| {
                            perform_copy(&mut final_source, &mut output_file, output_path.clone())
                                .with_context(|| format!("when extracting from [{source_path:?}]({:?}) to [{}]", archive_hash_path, output_path.display()))
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

use {
    anyhow::{Context, Result},
    image_dds::{self, ddsfile::Dds, image::DynamicImage, mip_dimension, SurfaceRgba32Float},
    std::io::{Read, Write},
    tap::prelude::*,
    tracing::instrument,
};

#[instrument]
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
        DXGIFormat::BC2_UNORM => {
            tracing::warn!("BC2 is not supported, using BC3 instead");
            image_dds::ImageFormat::BC3RgbaUnorm
        }
        DXGIFormat::BC2_UNORM_SRGB => {
            tracing::warn!("BC2 is not supported, using BC3 instead");
            image_dds::ImageFormat::BC3RgbaUnormSrgb
        }
        unsupported => anyhow::bail!("{unsupported:?} is not supported"),
    }
    .pipe(Ok)
}

#[tracing::instrument(skip(input, output))]
pub fn resize_dds<R, W>(
    input: &mut R,
    target_width: u32,
    target_height: u32,
    target_format: image_dds::ImageFormat,
    target_mipmaps: u32,
    output: &mut W,
) -> Result<()>
where
    R: Read,
    W: Write,
{
    Dds::read(input)
        .context("reading dds file")
        .and_then(|dds| {
            image_dds::Surface::from_dds(&dds)
                .context("reading surface")
                .and_then(|surface| {
                    surface
                        .decode_rgbaf32()
                        .context("decoding rgbaf32")
                        .and_then(|decoded| {
                            // note to self: layer == face
                            std::iter::once(())
                                .flat_map(|_| (0..decoded.layers))
                                .flat_map(|layer| (0..decoded.depth).map(move |depth| (layer, depth)))
                                .map(|(layer, depth)| {
                                    // we will regenerate mipmaps
                                    const MIPMAP: u32 = 0;
                                    decoded
                                        .get(layer, depth, MIPMAP)
                                        .context("getting the chunk from decoded surface")
                                        .and_then(|data| {
                                            image_dds::image::ImageBuffer::from_raw(
                                                mip_dimension(surface.width, MIPMAP),
                                                mip_dimension(surface.height, MIPMAP),
                                                data.to_vec(),
                                            )
                                            .context("loading part into an ImageBuffer failed")
                                        })
                                        .map(DynamicImage::ImageRgba32F)
                                        .map(|image| image.resize_exact(target_width, target_height, image_dds::image::imageops::FilterType::Lanczos3))
                                        .map(|resized| resized.into_rgba32f())
                                        .with_context(|| format!("processing part layer={layer}, depth={depth}, mipmap={MIPMAP}"))
                                })
                                .try_fold(Vec::new(), |mut acc, part| {
                                    part.map(|part| {
                                        acc.extend(part.into_vec());
                                        acc
                                    })
                                })
                                .with_context(|| {
                                    format!(
                                        "resizing all parts of dds (layers={}, depths={}, mipmaps={}, image_format={:?}, data_len=[{}])",
                                        surface.layers,
                                        surface.depth,
                                        surface.mipmaps,
                                        surface.image_format,
                                        surface.data.len()
                                    )
                                })
                        })
                        .map(|data| SurfaceRgba32Float {
                            data,
                            width: target_width,
                            height: target_height,
                            depth: surface.depth,
                            layers: surface.layers,
                            // this newly created surface only has 1 mipmap, the encoder will generate the desired amount
                            mipmaps: 1,
                        })
                        .and_then(|resized_surface| {
                            resized_surface
                                .encode(target_format, image_dds::Quality::Normal, image_dds::Mipmaps::GeneratedExact(target_mipmaps))
                                .context("reencoding surface")
                        })
                })
        })
        .and_then(|reencoded| reencoded.to_dds().context("creating a dds file"))
        .and_then(|dds| dds.write(output).context("writing dds file to output"))
        .context("recompressing/resizing a dds file")
}

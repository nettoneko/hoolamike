use {
    crate::modlist_json::image_format::DXGIFormat,
    anyhow::{Context, Result},
    ddsfile::{AlphaMode, D3D10ResourceDimension, Dds, DxgiFormat},
    image::{GenericImageView, ImageBuffer, Pixel},
    intel_tex::{bc1, bc3, bc6h, bc7},
    std::io::{BufReader, Read, Write},
    tap::{Pipe, Tap},
    tracing::{info, warn},
};

#[allow(non_camel_case_types)]
enum OutputFormat {
    BC1_TYPELESS,
    BC1_UNORM,
    BC1_UNORM_SRGB,
    BC3_TYPELESS,
    BC3_UNORM,
    BC3_UNORM_SRGB,
    BC6H_TYPELESS,
    BC6H_UF16,
    BC6H_SF16,
    BC7_TYPELESS,
    BC7_UNORM,
    BC7_UNORM_SRGB,
}

impl OutputFormat {
    fn match_output_format(target_format: DXGIFormat) -> Option<Self> {
        match target_format {
            DXGIFormat::BC1_TYPELESS => Some(Self::BC1_TYPELESS),
            DXGIFormat::BC1_UNORM => Some(Self::BC1_UNORM),
            DXGIFormat::BC1_UNORM_SRGB => Some(Self::BC1_UNORM_SRGB),
            DXGIFormat::BC3_TYPELESS => Some(Self::BC3_TYPELESS),
            DXGIFormat::BC3_UNORM => Some(Self::BC3_UNORM),
            DXGIFormat::BC3_UNORM_SRGB => Some(Self::BC3_UNORM_SRGB),
            DXGIFormat::BC6H_TYPELESS => Some(Self::BC6H_TYPELESS),
            DXGIFormat::BC6H_UF16 => Some(Self::BC6H_UF16),
            DXGIFormat::BC6H_SF16 => Some(Self::BC6H_SF16),
            DXGIFormat::BC7_TYPELESS => Some(Self::BC7_TYPELESS),
            DXGIFormat::BC7_UNORM => Some(Self::BC7_UNORM),
            DXGIFormat::BC7_UNORM_SRGB => Some(Self::BC7_UNORM_SRGB),
            _ => None,
        }
    }
}

macro_rules! spanned {
    ($expr:expr) => {
        tracing::info_span!(stringify!($expr)).in_scope(|| $expr)
    };
}

#[tracing::instrument(skip(input, output))]
pub fn resize_dds<R, W>(input: &mut R, target_width: u32, target_height: u32, target_format: DXGIFormat, target_mipmaps: u32, output: &mut W) -> Result<()>
where
    R: Read,
    W: Write,
{
    OutputFormat::match_output_format(target_format)
        .with_context(|| format!("{target_format:?} is not supported by intel tex"))
        .and_then(|output_format| {
            warn!("trying experimental intel texture recompression library! if it fails it will fall back to slower microsoft directxtex");
            spanned!(Dds::read(input))
                .context("reading dds file")
                .and_then(|dds_file| {
                    spanned!(image::ImageReader::new(BufReader::new(std::io::Cursor::new(&dds_file.data))).with_guessed_format())
                        .context("reading image data")
                        .and_then(|image| spanned!(image.decode().context("bad image")))
                        .and_then(|image| {
                            image.dimensions().pipe(|(width, height)| {
                                ImageBuffer::new(width, height)
                                    .tap_mut(|rgba_img| {
                                        (0..width)
                                            .flat_map(|x| (0..height).map(move |y| (x, y)))
                                            .map(|(x, y)| (x, y, image.get_pixel(x, y).to_rgba()))
                                            .for_each(|(x, y, pixel)| {
                                                rgba_img.put_pixel(x, y, pixel);
                                            })
                                    })
                                    .pipe(|rgba_img| {
                                        intel_tex::divide_up_by_multiple(width * height, 16)
                                            .tap(|block_count| info!("block count: {block_count}"))
                                            .pipe(|_| {
                                                let mip_count = dds_file.header.mip_map_count;
                                                let array_layers = dds_file
                                                    .header10
                                                    .as_ref()
                                                    .map(|a| a.array_size)
                                                    .unwrap_or(1);
                                                let caps2 = dds_file.header.caps2;
                                                let is_cubemap = false;
                                                let resource_dimension = dds_file
                                                    .header10
                                                    .as_ref()
                                                    .map(|h| h.resource_dimension)
                                                    .unwrap_or(D3D10ResourceDimension::Texture2D);
                                                let alpha_mode = dds_file
                                                    .header10
                                                    .as_ref()
                                                    .map(|h| h.alpha_mode)
                                                    .unwrap_or(AlphaMode::Opaque);
                                                let depth = dds_file.header.depth.unwrap_or(1);

                                                let is_opaque = match alpha_mode {
                                                    AlphaMode::Opaque => true,
                                                    AlphaMode::Unknown => false,
                                                    AlphaMode::Straight => false,
                                                    AlphaMode::PreMultiplied => false,
                                                    AlphaMode::Custom => false,
                                                };
                                                Dds::new_dxgi(ddsfile::NewDxgiParams {
                                                    height,
                                                    width,
                                                    depth: Some(depth),
                                                    format: DxgiFormat::BC7_UNorm,
                                                    mipmap_levels: mip_count,
                                                    array_layers: Some(array_layers),
                                                    caps2: Some(caps2),
                                                    is_cubemap,
                                                    resource_dimension,
                                                    alpha_mode,
                                                })
                                                .context("creating dds file")
                                                .and_then(|mut dds| {
                                                    intel_tex::RgbaSurface {
                                                        width,
                                                        height,
                                                        stride: width * 4,
                                                        data: &rgba_img,
                                                    }
                                                    .pipe(|surface| {
                                                        dds.get_mut_data(0 /* layer */)
                                                            .context("layers")
                                                            .map(|output_layer| match output_format {
                                                                OutputFormat::BC7_TYPELESS => {
                                                                    spanned!(bc7::compress_blocks_into(
                                                                        &match is_opaque {
                                                                            true => bc7::opaque_ultra_fast_settings(),
                                                                            false => bc7::alpha_ultra_fast_settings(),
                                                                        },
                                                                        &surface,
                                                                        output_layer,
                                                                    ));
                                                                }
                                                                OutputFormat::BC1_TYPELESS => {
                                                                    spanned!(bc1::compress_blocks_into(&surface, output_layer));
                                                                }
                                                                OutputFormat::BC1_UNORM => {
                                                                    spanned!(bc1::compress_blocks_into(&surface, output_layer));
                                                                }
                                                                OutputFormat::BC1_UNORM_SRGB => {
                                                                    spanned!(bc1::compress_blocks_into(&surface, output_layer));
                                                                }
                                                                OutputFormat::BC3_TYPELESS => {
                                                                    spanned!(bc3::compress_blocks_into(&surface, output_layer));
                                                                }
                                                                OutputFormat::BC3_UNORM => {
                                                                    spanned!(bc3::compress_blocks_into(&surface, output_layer));
                                                                }
                                                                OutputFormat::BC3_UNORM_SRGB => {
                                                                    spanned!(bc3::compress_blocks_into(&surface, output_layer));
                                                                }
                                                                OutputFormat::BC6H_TYPELESS => {
                                                                    spanned!(bc6h::compress_blocks_into(
                                                                        &match is_opaque {
                                                                            true => bc6h::very_fast_settings(),
                                                                            false => bc6h::very_fast_settings(),
                                                                        },
                                                                        &surface,
                                                                        output_layer,
                                                                    ));
                                                                }
                                                                OutputFormat::BC6H_UF16 => {
                                                                    spanned!(bc6h::compress_blocks_into(
                                                                        &match is_opaque {
                                                                            true => bc6h::very_fast_settings(),
                                                                            false => bc6h::very_fast_settings(),
                                                                        },
                                                                        &surface,
                                                                        output_layer,
                                                                    ));
                                                                }
                                                                OutputFormat::BC6H_SF16 => {
                                                                    spanned!(bc6h::compress_blocks_into(
                                                                        &match is_opaque {
                                                                            true => bc6h::very_fast_settings(),
                                                                            false => bc6h::very_fast_settings(),
                                                                        },
                                                                        &surface,
                                                                        output_layer,
                                                                    ));
                                                                }
                                                                OutputFormat::BC7_UNORM => {
                                                                    spanned!(bc7::compress_blocks_into(
                                                                        &match is_opaque {
                                                                            true => bc7::opaque_ultra_fast_settings(),
                                                                            false => bc7::alpha_ultra_fast_settings(),
                                                                        },
                                                                        &surface,
                                                                        output_layer,
                                                                    ));
                                                                }
                                                                OutputFormat::BC7_UNORM_SRGB => {
                                                                    spanned!(bc7::compress_blocks_into(
                                                                        &match is_opaque {
                                                                            true => bc7::opaque_ultra_fast_settings(),
                                                                            false => bc7::alpha_ultra_fast_settings(),
                                                                        },
                                                                        &surface,
                                                                        output_layer,
                                                                    ));
                                                                }
                                                            })
                                                    })
                                                })
                                            })
                                    })
                            })
                        })
                        .and_then(|_| dds_file.write(output).context("writing dds file"))
                })
        })
}

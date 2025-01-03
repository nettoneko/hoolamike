use {
    crate::{modlist_json::image_format::DXGIFormat, progress_bars_v2::IndicatifWrapIoExt},
    anyhow::{Context, Result},
    directxtex::{self, TexMetadata, DDS_FLAGS, DXGI_FORMAT, TEX_COMPRESS_FLAGS, TEX_FILTER_FLAGS, TEX_THRESHOLD_DEFAULT},
    num::ToPrimitive,
    std::io::{Read, Write},
    tap::prelude::*,
};

pub mod dxgi_format_mapping;

#[tracing::instrument(skip(input, output))]
pub fn resize_dds<R, W>(input: &mut R, target_width: u32, target_height: u32, target_format: DXGIFormat, target_mipmaps: u32, output: &mut W) -> Result<()>
where
    R: Read,
    W: Write,
{
    let dds_flags = DDS_FLAGS::DDS_FLAGS_PERMISSIVE;
    let tex_filter_flags = TEX_FILTER_FLAGS::TEX_FILTER_TRIANGLE;
    let tex_compress_flags = TEX_COMPRESS_FLAGS::TEX_COMPRESS_DEFAULT;
    let target_format = self::dxgi_format_mapping::map_dxgi_format(target_format);

    Vec::new()
        .pipe(|mut buf| input.read_to_end(&mut buf).map(|_| buf))
        .context("reading bytes")
        .and_then(|bytes| {
            let tex_metadata = TexMetadata::from_dds(&bytes, dds_flags, None).context("reading tex metadata")?;
            Ok(())
                .and_then(|_| {
                    directxtex::ScratchImage::load_dds(&bytes, dds_flags, None, None)
                        .context("loading dds")
                        .and_then(|image| {
                            image
                                .decompress(DXGI_FORMAT::DXGI_FORMAT_R32G32B32A32_FLOAT)
                                .context("decompressing image")
                        })
                        .context("loading image")
                        .and_then(|image| {
                            Ok(image)
                                .and_then(|image| {
                                    image
                                        .resize(
                                            target_width.to_usize().context("bad target_width")?,
                                            target_height.to_usize().context("bad target_height")?,
                                            tex_filter_flags,
                                        )
                                        .context("resizing")
                                })
                                .and_then(|resized| {
                                    resized
                                        .generate_mip_maps(tex_filter_flags, target_mipmaps.to_usize().context("bad target_mipmaps")?)
                                        .context("generating mip maps")
                                })
                        })
                        .context("modifying image")
                        .and_then(|image| {
                            image
                                .compress(target_format, tex_compress_flags, TEX_THRESHOLD_DEFAULT)
                                .context("compressing image")
                        })
                        .context("preparing image for dump")
                })
                .and_then(|image| {
                    image
                        .images()
                        .pipe(|images| {
                            directxtex::save_dds(
                                images,
                                &tex_metadata.tap_mut(|metadata| {
                                    metadata.width = target_width as _;
                                    metadata.height = target_height as _;
                                    metadata.mip_levels = target_mipmaps as _;
                                    metadata.format = target_format;
                                }),
                                dds_flags,
                            )
                        })
                        .context("saving dds image to bytes")
                })
                .and_then(|blob| {
                    std::io::copy(
                        &mut tracing::Span::current().wrap_read(blob.buffer().len() as _, std::io::Cursor::new(blob.buffer())),
                        output,
                    )
                    .context("writing dds file")
                })
        })
        .map(|wrote| tracing::debug!("wrote {wrote} bytes"))
}

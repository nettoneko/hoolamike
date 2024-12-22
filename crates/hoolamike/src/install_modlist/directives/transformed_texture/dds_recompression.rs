use {
    anyhow::{Context, Result},
    crossbeam::epoch::Pointable,
    image_dds::{
        self,
        ddsfile::{DataFormat, Dds, DxgiFormat},
        image::{DynamicImage, GenericImage, ImageBuffer},
        ImageFormat,
        SurfaceRgba32Float,
    },
    std::{
        borrow::BorrowMut,
        io::{BufReader, Cursor, Read, Write},
    },
};

pub fn recompress<R, W>(input: &mut R, width: u32, height: u32, mip_maps: u32, format: ImageFormat, output: &mut W, leave_open: bool) -> Result<()>
where
    R: Read,
    W: Write,
{
    image_dds::ddsfile::Dds::read(input)
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
                                .flat_map(|(layer, depth)| (0..decoded.mipmaps).map(move |mipmap| (layer, depth, mipmap)))
                                .map(|(layer, depth, mipmap)| {
                                    decoded
                                        .get(layer, depth, mipmap)
                                        .with_context(|| format!("reading data for layer={layer}, depth={depth}, mipmap={mipmap}"))
                                        .and_then(|data| image_dds::image::ImageBuffer::from_vec(width, height, data.to_vec()).context("creating a buffer"))
                                        .map(|image| DynamicImage::ImageRgba32F(image))
                                        .map(|image| image.resize_exact(width, height, image_dds::image::imageops::FilterType::Lanczos3))
                                        .map(|resized| resized.into_rgba32f())
                                })
                                .try_fold(Vec::new(), |mut acc, part| {
                                    part.map(|part| {
                                        acc.extend(part.into_vec());
                                        acc
                                    })
                                })
                        })
                        .map(|data| SurfaceRgba32Float {
                            data,
                            width,
                            height,
                            depth: surface.depth,
                            layers: surface.layers,
                            mipmaps: surface.mipmaps,
                        })
                        .and_then(|surface| {
                            surface
                                .encode(format, image_dds::Quality::Normal, image_dds::Mipmaps::FromSurface)
                                .context("reencoding surface")
                        })
                })
        })
        .and_then(|reencoded| reencoded.to_dds().context("creating a dds file"))
        .and_then(|dds| dds.write(output).context("writing dds file to output"))
}

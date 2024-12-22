use {
    anyhow::{Context, Result},
    image_dds::{
        self,
        ddsfile::{Dds, DxgiFormat},
        image::DynamicImage,
    },
    std::io::{Read, Write},
};
// Our rough 'recompress' function
pub fn recompress<R, W>(input: &mut R, width: u32, height: u32, mip_maps: u32, format: String, output: &mut W, leave_open: bool) -> Result<()>
where
    R: Read,
    W: Write,
{
    // 1) Read the entire stream into memory (or parse chunk by chunk if you prefer)
    let mut buffer = Vec::new();
    input.read_to_end(&mut buffer).context("reading input")?;

    // 2) Load (parse) the DDS file
    let dds_file = Dds::read(&mut std::io::Cursor::new(&buffer)).context("parsing dds file")?;
    let decoded_faces = (0..(mip_maps))
        .map(|mip_map| {
            image_dds(&dds_file, mip_map)
                .with_context(|| format!("reading mip_map {mip_map}"))
                .map(DynamicImage::ImageRgba8)
                .map(|face| face.resize_exact(width, height, image_dds::image::imageops::FilterType::Gaussian))
        })
        .collect::<Result<Vec<_>>>()
        .context("reading dds file mipmaps")?;

    // 3) If we don't want to keep the input open, we can drop it here
    // NOT NEEDED

    // 4) Decode each face, resize, collect them in a vector
    let mut decoded_faces = Vec::with_capacity(mip_maps as usize);

    // For demonstration, let's assume we figure out the original format from somewhere
    let orig_format = DxgiFormat::BC1Unorm; // placeholder
    for face in dds_file.faces.iter() {
        if face.mip_maps.is_empty() {
            // In a real scenario you might skip or return an error
            continue;
        }
        let first_mip = &face.mip_maps[0];

        let mut decoded = decode_raw_to_image_rgba32_async(&first_mip.data, face.width, face.height, orig_format).await?;

        // 5) Resize the image
        resize_image(&mut decoded, width, height);

        decoded_faces.push(decoded);
    }

    // 6) Set up the BC encoder
    let mut encoder = BcEncoder::new();
    encoder.set_format(format);
    encoder.set_generate_mipmaps(true);

    // If mip_maps == 0, let the encoder generate all it wants, otherwise restrict
    if mip_maps == 0 {
        encoder.set_max_mipmap_level(-1); // meaning unlimited
    } else {
        encoder.set_max_mipmap_level(mip_maps as i32);
    }

    // 7) Encode to DDS
    let face_count = decoded_faces.len();
    let encoded_bytes = match face_count {
        1 => {
            // 2D texture
            encoder.encode_to_dds_async(&decoded_faces[0]).await?
        }
        6 => {
            // Cube map
            encoder.encode_cube_map_to_dds_async(&decoded_faces).await?
        }
        _ => {
            // Not implemented
            return Err(format!("Can't encode DDS with {} faces", face_count).into());
        }
    };

    // 8) Write the result to the output stream
    output.write_all(&encoded_bytes).await?;

    // 9) If not leaving open, flush/close the output
    if !leave_open {
        // In Rust, the drop of 'output' can close it, but you can also explicitly flush.
        output.flush().await?;
    }

    Ok(())
}

use super::*;

#[test]
fn test_stat_example_file() -> Result<()> {
    let output = Wrapped7Zip::find_bin()?.query_file_info(Path::new("./test-data/example-1.rar"))?;
    println!("{output}");
    assert!(output.contains("20:58:56"));
    Ok(())
}

#[tokio::test]
async fn test_extract_example_files() -> Result<()> {
    let handler = Wrapped7Zip::find_bin()?;
    [
        //
        ("test-data/example-small-file.7z", "small-file.json"),
        (
            "test-data/example-small-file.7z",
            "long path/with some whitespace/lets add some more/small-file.json",
        ),
    ]
    .into_iter()
    .try_for_each(|(archive, file)| {
        handler
            .open_file(Path::new(archive))
            .and_then(|archive| archive.get_file(Path::new(file)))
            .and_then(|(ListOutputEntry { size, .. }, mut file)| {
                std::io::copy(&mut file, &mut std::io::sink())
                    .context("decompressing file")
                    .and_then(|read| {
                        read.eq(&size)
                            .then_some(())
                            .with_context(|| format!("expected size {size}, found {read}"))
                    })
            })
            .with_context(|| format!("testing {archive} -> {file}"))
    })
}
#[tokio::test]
async fn extract_example_file() -> Result<()> {
    let archive = Wrapped7Zip::find_bin()?.open_file(Path::new("./test-data/example-1.rar"))?;
    let files = archive.list_files()?;
    let (_, mut file) = archive.get_file(&files[0].path)?;
    let mut out = Vec::new();

    let read = std::io::copy(&mut file, &mut std::io::Cursor::new(&mut out)).context("copy failed")?;

    assert_eq!(files[0].size, read, "read is wrong");
    assert_eq!(files[0].size, out.len() as u64, "output buffer length is wrong");

    Ok(())
}

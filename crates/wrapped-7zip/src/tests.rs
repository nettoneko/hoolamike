use super::*;

#[test]
fn test_stat_example_file() -> Result<()> {
    let file = which::which("7z").context("checking binary path")?;
    let output = Wrapped7Zip::new(&file)?.query_file_info(Path::new("./test-data/example-1.rar"))?;
    println!("{output}");
    assert!(output.contains("20:58:56"));
    Ok(())
}

#[test]
fn extract_example_file() -> Result<()> {
    let file = which::which("7z").context("checking binary path")?;
    let archive = Wrapped7Zip::new(&file)?.open_file(Path::new("./test-data/example-1.rar"))?;
    let files = archive.list_files()?;
    let mut file = archive.get_file(&files[0].path)?;
    let mut out = Vec::new();

    let read = std::io::copy(&mut file, &mut std::io::Cursor::new(&mut out)).context("copy failed")?;

    assert_eq!(files[0].size, read, "read is wrong");
    assert_eq!(files[0].size, out.len() as u64, "output buffer length is wrong");

    Ok(())
}

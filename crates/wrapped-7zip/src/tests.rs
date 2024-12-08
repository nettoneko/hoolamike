use super::*;
#[test]
fn test_example_file() -> Result<()> {
    let file = which::which("7z").context("checking binary path")?;
    let output = Wrapped7Zip::new(&file)?.query_file_info(Path::new("./test-data/example-1.rar"))?;
    println!("{output}");
    assert!(output.contains("kB"));
    Ok(())
}

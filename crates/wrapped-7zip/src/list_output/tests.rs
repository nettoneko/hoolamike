use {super::*, pretty_assertions::assert_eq};

#[test_log::test]
fn test_example_1() -> Result<()> {
    const EXAMPLE_1: &str = r#"
7-Zip [64] 17.05 : Copyright (c) 1999-2021 Igor Pavlov : 2017-08-28
p7zip Version 17.05 (locale=en_US.UTF-8,Utf16=on,HugeFiles=on,64 bits,32 CPUs x64)

Scanning the drive for archives:
1 file, 3911 bytes (4 KiB)

Listing archive: ./test-data/example-1.rar

--
Path = ./test-data/example-1.rar
Type = Rar
Physical Size = 3911
Solid = -
Blocks = 3
Multivolume = -
Volumes = 1

   Date      Time    Attr         Size   Compressed  Name
------------------- ----- ------------ ------------  ------------------------
2017-08-03 20:58:56 ....A         3008         3008  Data/PD_LowerWeapon - Main.ba2
2017-08-03 20:00:05 ....A         1831          708  Data/PD_LowerWeapon.esp
2017-08-03 20:58:56 D....            0            0  Data
------------------- ----- ------------ ------------  ------------------------
2017-08-03 20:58:56               4839         3716  2 files, 1 folders
    "#;

    let expected = vec![
        ListOutputEntry {
            date: "2017-08-03".parse()?,
            time: "20:58:56".parse()?,
            attr: "....A".parse()?,
            size: 3008,
            compressed: 3008,
            name: "Data/PD_LowerWeapon - Main.ba2".parse()?,
        },
        ListOutputEntry {
            date: "2017-08-03".parse()?,
            time: "20:00:05".parse()?,
            attr: "....A".parse()?,
            size: 1831,
            compressed: 708,
            name: "Data/PD_LowerWeapon.esp".parse()?,
        },
        ListOutputEntry {
            date: "2017-08-03".parse()?,
            time: "20:58:56".parse()?,
            attr: "D....".parse()?,
            size: 0,
            compressed: 0,
            name: "Data".into(),
        },
    ];
    assert_eq!(ListOutput { entries: expected }, ListOutput::from_str(EXAMPLE_1).unwrap());
    Ok(())
}

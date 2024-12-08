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
            compressed: Some(3008),
            name: "Data/PD_LowerWeapon - Main.ba2".parse()?,
        },
        ListOutputEntry {
            date: "2017-08-03".parse()?,
            time: "20:00:05".parse()?,
            attr: "....A".parse()?,
            size: 1831,
            compressed: Some(708),
            name: "Data/PD_LowerWeapon.esp".parse()?,
        },
        ListOutputEntry {
            date: "2017-08-03".parse()?,
            time: "20:58:56".parse()?,
            attr: "D....".parse()?,
            size: 0,
            compressed: Some(0),
            name: "Data".into(),
        },
    ];
    assert_eq!(ListOutput { entries: expected }, ListOutput::from_str(EXAMPLE_1).unwrap());
    Ok(())
}

#[test_log::test]
fn test_example_2() -> Result<()> {
    const EXAMPLE_2: &str = r#"Copyright (c) 1999-2021 Igor Pavlov : 2017-08-28
p7zip Version 17.05 (locale=en_US.UTF-8,Utf16=on,HugeFiles=on,64 bits,32 CPUs x64)

Scanning the drive for archives:
1 file, 37626 bytes (37 KiB)

Listing archive: /home/niedzwiedz/Games/modding/hoolamike/playground/downloads/Cut SFX Restored-58353-1-0-0-1645551622.7z

--
Path = /home/niedzwiedz/Games/modding/hoolamike/playground/downloads/Cut SFX Restored-58353-1-0-0-1645551622.7z
Type = 7z
Physical Size = 37626
Headers Size = 353
Method = LZMA:16
Solid = +
Blocks = 1

   Date      Time    Attr         Size   Compressed  Name
------------------- ----- ------------ ------------  ------------------------
2022-02-22 17:57:38 ....A         4116        37273  Elzee Cut SFX Restored.esp
2020-07-19 09:47:49 ....A         1211               scripts/PlayerWorkbenchScript.pex
2021-10-26 17:35:57 ....A        40534               Sound/FX/UI/UI_SneakAttack_01.xwm
2022-02-22 17:59:56 D....            0            0  Sound/FX/UI
2022-02-22 17:59:56 D....            0            0  Sound/FX
2022-02-22 17:59:56 D....            0            0  Sound
2022-02-22 18:01:43 D....            0            0  scripts
------------------- ----- ------------ ------------  ------------------------
2022-02-22 18:01:43              45861        37273  3 files, 4 folders"#;

    let expected = vec![
        ListOutputEntry {
            date: "2022-02-22".parse()?,
            time: "17:57:38".parse()?,
            attr: "....A".parse()?,
            size: 4116,
            compressed: Some(37273),
            name: "Elzee Cut SFX Restored.esp".parse()?,
        },
        ListOutputEntry {
            date: "2020-07-19".parse()?,
            time: "09:47:49".parse()?,
            attr: "....A".parse()?,
            size: 1211,
            compressed: None,
            name: "scripts/PlayerWorkbenchScript.pex".parse()?,
        },
        ListOutputEntry {
            date: "2021-10-26".parse()?,
            time: "17:35:57".parse()?,
            attr: "....A".parse()?,
            size: 40534,
            compressed: None,
            name: "Sound/FX/UI/UI_SneakAttack_01.xwm".parse()?,
        },
        ListOutputEntry {
            date: "2022-02-22".parse()?,
            time: "17:59:56".parse()?,
            attr: "D....".parse()?,
            size: 0,
            compressed: Some(0),
            name: "Sound/FX/UI".into(),
        },
        ListOutputEntry {
            date: "2022-02-22".parse()?,
            time: "17:59:56".parse()?,
            attr: "D....".parse()?,
            size: 0,
            compressed: Some(0),
            name: "Sound/FX".into(),
        },
        ListOutputEntry {
            date: "2022-02-22".parse()?,
            time: "17:59:56".parse()?,
            attr: "D....".parse()?,
            size: 0,
            compressed: Some(0),
            name: "Sound".into(),
        },
        ListOutputEntry {
            date: "2022-02-22".parse()?,
            time: "18:01:43".parse()?,
            attr: "D....".parse()?,
            size: 0,
            compressed: Some(0),
            name: "scripts".into(),
        },
    ];
    assert_eq!(ListOutput { entries: expected }, ListOutput::from_str(EXAMPLE_2).unwrap());
    Ok(())
}


#[test_log::test]
fn test_example_3() -> Result<()> {
    const EXAMPLE_3: &str = r#"7-Zip [64] 17.05 : Copyright (c) 1999-2021 Igor Pavlov : 2017-08-28
p7zip Version 17.05 (locale=en_US.UTF-8,Utf16=on,HugeFiles=on,64 bits,32 CPUs x64)

Scanning the drive for archives:
1 file, 9602252 bytes (9378 KiB)

Listing archive: /home/niedzwiedz/Games/modding/hoolamike/playground/downloads/Power Armor to the People-50819-2-10-0-1717383800.zip

--
Path = /home/niedzwiedz/Games/modding/hoolamike/playground/downloads/Power Armor to the People-50819-2-10-0-1717383800.zip
Type = zip
Physical Size = 9602252

   Date      Time    Attr         Size   Compressed  Name
------------------- ----- ------------ ------------  ------------------------
2024-05-13 23:26:18 .....        72632        69207  fomod\Abandoned Power Armor.jpg
2021-06-01 19:49:32 .....        90960        87882  fomod\Captain Cosmos.jpg
2021-04-17 14:32:48 .....       182689       181851  fomod\Classic Advanced Power Armor.jpg
2024-05-13 23:26:18 .....       107173       103962  fomod\Combat Power Armor.jpg
2024-05-13 23:26:18 .....       158010       154912  fomod\Consistent Power Armor Overhaul.jpg
2024-05-13 23:26:18 .....       110631       107518  fomod\Edmond's Power Armor Backpacks 2024.jpg
2024-05-13 23:26:18 .....       137383       134255  fomod\Enclave Power Armor.jpg
2021-04-17 14:32:48 .....       105105       104460  fomod\Enclave X-02 - BoS.jpg
2021-03-26 01:49:52 .....        91876        91220  fomod\Enclave X-02.jpg
2021-03-26 01:49:30 .....       130234       129574  fomod\Excavator Power Armor.jpg
2021-06-01 19:49:32 .....       113280       112623  fomod\Gunner Outfit Pack.jpg
2021-06-01 19:49:32 .....       135032       134461  fomod\Gunners vs Minutemen (Creation Club).jpg
2021-10-25 20:35:28 .....        98853        95677  fomod\Hellcat Power Armor.jpg
2021-04-17 14:32:48 .....       180807       180032  fomod\Hellfire Power Armor (Creation Club).jpg
2021-04-17 14:32:48 .....       107506       106825  fomod\Hellfire X-03 - BoS.jpg
2021-03-26 01:49:08 .....       175571       174736  fomod\Hellfire X-03.jpg
2021-06-01 19:49:32 .....       165873       165057  fomod\Horse Power Armor (Creation Club).jpg
2024-05-13 23:26:20 .....         2926          958  fomod\info.xml
2021-03-26 01:48:46 .....       128534       127962  fomod\Institute Heavy Weapons Patch.jpg
2021-03-26 01:48:24 .....       178926       178329  fomod\Institute Power Armor.jpg
2024-05-13 23:26:18 .....       104593       101482  fomod\Liberty Power Armor.jpg
2021-06-01 19:49:32 .....        96833        93738  fomod\Midwest Power Armor Evolution.jpg
2024-05-13 23:26:18 .....        97685        94500  fomod\Midwest Power Armor Revolution.jpg
2021-10-16 14:38:48 .....       160718       157402  fomod\Minutemen Paint Job.jpg
2024-06-03 02:52:32 .....       227850        13489  fomod\ModuleConfig.xml
2024-05-13 23:26:18 .....        81053        77782  fomod\Nighthawk Power Armor.jpg
2021-10-16 14:38:50 .....       103464       100365  fomod\Overboss Power Armor - No Chains.jpg
2024-05-13 23:26:18 .....       144267       141132  fomod\Power Armor Paints.jpg
2024-05-13 23:26:18 .....       120288       117235  fomod\Power Armor Pouches.jpg
2021-03-26 01:47:28 .....       151024       150376  fomod\Power Armor to the People Cover Image.jpg
2021-03-26 01:45:58 .....       131721       131082  fomod\Raider Overhaul Patch.jpg
2024-05-13 23:26:18 .....        45104        41546  fomod\Red Shift Power Armor.jpg
2021-03-26 01:45:32 .....        89081        87247  fomod\Redistribute Power Armor.jpg
2021-03-26 01:45:06 .....       123780       123099  fomod\Rusty T-51.jpg
2021-10-16 14:38:50 .....        96563        93035  fomod\Scavenger Mismatched Power Armor.jpg
2024-05-13 23:26:18 .....       114154       110847  fomod\SE-01 Power Armor.jpg
2021-10-25 20:35:28 .....       147508       144434  fomod\Settler Vigilante Power Armor.jpg
2024-05-13 23:26:18 .....       111385       108183  fomod\Soviet Power Armor.jpg
2024-05-13 23:26:18 .....        67160        63887  fomod\Spartan Battle Suit.jpg
2024-05-13 23:26:20 .....        88262        85062  fomod\Submersible Power Armor Redux.jpg
2024-05-13 23:26:20 .....       129098       126117  fomod\SuperAlloys Cohesion.jpg
2024-05-13 23:26:20 .....       166182       162870  fomod\Synth Power Armor.jpg
2024-05-13 23:26:20 .....        94346        90863  fomod\T-47R.jpg
2024-05-13 23:26:20 .....        81179        77814  fomod\T-49.jpg
2021-03-26 01:44:36 .....        82506        81773  fomod\T-51c.jpg
2024-05-13 23:26:20 .....       136799       133774  fomod\T-60 Equipment.jpg
2024-05-13 23:26:20 .....       106535       103429  fomod\T-65 - BoS.jpg
2024-05-13 23:26:20 .....       102456        99402  fomod\T-65 - East BoS Paint (Red).jpg
2024-05-13 23:26:20 .....       111930       108772  fomod\T-65 - East BoS Paint.jpg
2024-05-13 23:26:20 .....       108336       105239  fomod\T-65 - Gunner Paint.jpg
2021-03-26 01:43:56 .....       131270       130648  fomod\T-65.jpg
2024-05-13 23:26:20 .....       125048       121832  fomod\Tribal Power Armor.jpg
2024-05-13 23:26:20 .....       111084       108003  fomod\Tumbajamba's Raider Power Armor.jpg
2024-05-13 23:26:20 .....       104463       101291  fomod\Ultracite Power Armor - East BoS Paint.jpg
2021-03-26 01:43:30 .....       146652       145938  fomod\Ultracite Power Armor.jpg
2021-06-01 19:49:32 .....        96971        93543  fomod\Vault-Tec Power Armor.jpg
2024-05-13 23:26:20 .....       167154       163925  fomod\Visual Tesla Coils.jpg
2021-04-17 14:32:48 .....       100651        99868  fomod\X-01 - BoS.jpg
2021-03-26 01:43:06 .....       117501       116499  fomod\X-01 Tesla Upgrade Kit.jpg
2021-10-25 20:35:28 .....       150990       147958  fomod\X-02 Black Devil.jpg
2021-04-17 14:32:48 .....       176091       175296  fomod\X-02 Power Armored (Creation Club).jpg
2024-06-03 02:58:08 D....            0            0  Content\Compatibility\
2024-06-03 02:58:08 D....            0            0  Content\Extensions\
2024-06-03 02:58:08 D....            0            0  Content\Power Armor Sets\
2021-05-06 11:09:36 .....         2220         1086  Content\Compatibility\1.x\Institute Power Armor - Power Armored Enemies - Institute Heavy Weaponry.esp
2023-01-02 20:06:34 .....        12879        10950  Content\Compatibility\1.x\Institute Power Armor - Power Armored Enemies.esp
2021-05-06 11:18:36 .....         4864         4050  Content\Compatibility\1.x\UltracitePA - Power Armored Enemies.esp
2021-06-01 19:49:32 .....         1522          739  Content\Compatibility\1.x\ESL Version\T-65 Redistribution.esp
2021-06-05 13:39:58 .....         1522          739  Content\Compatibility\1.x\ESP Version\T-65 Redistribution.esp
2021-06-01 19:49:32 .....         1110          636  Content\Compatibility\1.x\Scripts\LegendaryPowerArmor_InjectLegendaries.pex
2021-06-01 19:49:32 .....         1351          722  Content\Compatibility\1.x\Scripts\PAttP\InjectArmorIntoLeveledList.pex
2021-06-01 19:49:32 .....         1162          643  Content\Compatibility\1.x\Scripts\PAttP\InjectItemIntoLeveledList.pex
2024-06-03 02:58:08 D....            0            0  Content\Core\MCM\
2024-06-03 02:58:08 D....            0            0  Content\Core\Scripts\
2024-06-03 02:58:08 D....            0            0  Content\Core\Sound\
2024-05-09 21:44:02 .....       619967       286384  Content\Core\Power Armor to the People.esp
2024-06-03 02:58:08 D....            0            0  Content\Core\MCM\Config\
2024-05-13 23:26:06 .....       140144        10722  Content\Core\MCM\Config\Power Armor to the People\config.json
2024-05-13 23:26:06 .....         5935         2602  Content\Core\Scripts\PAttP\AbandonedPowerArmorHandler.pex
2024-05-13 23:26:06 .....         2662         1387  Content\Core\Scripts\PAttP\AbsorbAmmoOnHitMagicEffect.pex
2024-05-13 23:26:06 .....         1905         1013  Content\Core\Scripts\PAttP\AddItemPeriodicallyMagicEffect.pex
2024-05-13 23:26:06 .....          662          420  Content\Core\Scripts\PAttP\AddItemQuest.pex
2024-05-13 23:26:06 .....         6087         2858  Content\Core\Scripts\PAttP\AddLegendariesToVendor.pex
2021-06-01 19:49:32 .....         1087          620  Content\Core\Scripts\PAttP\AddLegendaryRules.pex
2024-05-13 23:26:06 .....         2613         1302  Content\Core\Scripts\PAttP\AddPerkToFollowersInPowerArmor.pex
2024-05-13 23:26:06 .....         5796         2691  Content\Core\Scripts\PAttP\AddSpellOnHitMagicEffect.pex
2024-05-13 23:26:06 .....         2215         1153  Content\Core\Scripts\PAttP\AddSpellOnPowerAttackMagicEffect.pex
2024-05-13 23:26:06 .....         1099          645  Content\Core\Scripts\PAttP\AddSpellToPlayerMagicEffect.pex
2024-05-13 23:26:06 .....         2727         1337  Content\Core\Scripts\PAttP\ApplyKeywordMagicEffect.pex
2024-05-13 23:26:06 .....         4970         2296  Content\Core\Scripts\PAttP\AttachLegendaryModtoPowerArmor.pex
2024-05-13 23:26:06 .....         2363         1164  Content\Core\Scripts\PAttP\ChangeGlobalVariablesMagicEffect.pex
2024-05-13 23:26:06 .....         8156         3386  Content\Core\Scripts\PAttP\ChangeJetpackSettingsQuest.pex
2024-05-13 23:26:06 .....        18464         6275  Content\Core\Scripts\PAttP\ConfigurationManager.pex
2024-05-13 23:26:06 .....         3557         1754  Content\Core\Scripts\PAttP\CryogeneratorEnemyQuest.pex
2024-05-13 23:26:06 .....        21294         7602  Content\Core\Scripts\PAttP\CustomLegendaryRulesQuest.pex
2024-05-13 23:26:06 .....         8461         3075  Content\Core\Scripts\PAttP\DetectAndInjectItems.pex
2024-05-13 23:26:06 .....         1774          950  Content\Core\Scripts\PAttP\EnableReferencesQuest.pex
2024-05-13 23:26:06 .....         5398         2498  Content\Core\Scripts\PAttP\InjectionManager.pex
2024-05-13 23:26:06 .....         3829         1746  Content\Core\Scripts\PAttP\InjectionQuest.pex
2021-06-01 19:49:32 .....         2557         1312  Content\Core\Scripts\PAttP\InjectItemIntoExternalLeveledList.pex
2024-05-13 23:26:06 .....         1407          755  Content\Core\Scripts\PAttP\InjectItemIntoPAttPLeveledList.pex
2024-05-13 23:26:06 .....         2526         1273  Content\Core\Scripts\PAttP\InjectItemsIntoPAttPLeveledLists.pex
2024-05-13 23:26:06 .....         2829         1390  Content\Core\Scripts\PAttP\InjectLegendaryPowerArmor.pex
2024-05-13 23:26:06 .....         2474         1220  Content\Core\Scripts\PAttP\LegendaryNamingRuleListener.pex
2024-05-13 23:26:06 .....         2071         1145  Content\Core\Scripts\PAttP\LegendaryPowerArmorManager.pex
2021-06-01 19:49:32 .....         3004         1445  Content\Core\Scripts\PAttP\LeveledEncounterQuest.pex
2024-05-13 23:26:06 .....         1757          907  Content\Core\Scripts\PAttP\RarePowerArmorVendorQuest.pex
2024-05-13 23:26:06 .....          582          370  Content\Core\Scripts\PAttP\RarePowerArmorVendorTravelPackage.pex
2021-06-01 19:49:32 .....         3682         1692  Content\Core\Scripts\PAttP\RedemptionMachine.pex
2024-05-13 23:26:06 .....         5361         2464  Content\Core\Scripts\PAttP\ReflectBulletsMagicEffect.pex
2024-05-13 23:26:06 .....         2685         1333  Content\Core\Scripts\PAttP\RegisterUniqueItems.pex
2024-05-13 23:26:06 .....         1240          740  Content\Core\Scripts\PAttP\RegisterUniqueItemsEffect.pex
2024-05-13 23:26:06 .....         1492          875  Content\Core\Scripts\PAttP\SpawnNPCOnHitMagicEffect.pex
2024-05-13 23:26:06 .....          641          425  Content\Core\Scripts\PAttP\StartQuestWhenRead.pex
2024-05-13 23:26:06 .....          965          592  Content\Core\Scripts\PAttP\TriggerLegendaryNamingRulesMerge.pex
2024-05-13 23:26:06 .....        17302         7305  Content\Core\Scripts\PAttP\UniqueItemManager.pex
2024-05-13 23:26:06 .....          769          459  Content\Core\Scripts\PAttP\UpdateJetpackSettingsMagicEffect.pex
2024-05-13 23:26:06 .....          804          507  Content\Core\Scripts\PAttP\UpdateLegendaryRules.pex
2024-05-13 23:26:06 .....         8654         3708  Content\Core\Scripts\PAttP\UpgradeManager.pex
2024-05-13 23:26:06 .....         4954         2286  Content\Core\Scripts\PAttP\VigilanteHelperQuest.pex
2024-06-03 02:58:10 D....            0            0  Content\Core\Sound\Voice\
2024-06-03 02:58:10 D....            0            0  Content\Core\Sound\Voice\Power Armor to the People.esp\
2024-05-13 23:26:06 .....        40266        39207  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\00000380_1.xwm
2024-05-13 23:26:06 .....        64840        62269  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\00000381_1.xwm
2024-05-13 23:26:06 .....        67074        65732  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\00000382_1.xwm
2024-05-13 23:26:06 .....        29096        26711  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\00000385_1.xwm
2024-05-13 23:26:06 .....        20160        17967  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\00000387_1.xwm
2024-05-13 23:26:06 .....        44734        42276  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\00000389_1.xwm
2024-05-13 23:26:06 .....        69308        68224  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\0000038B_1.xwm
2024-05-13 23:26:08 .....        82712        80023  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\0000038B_2.xwm
2024-05-13 23:26:08 .....        42500        41858  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\0000038B_3.xwm
2024-05-13 23:26:08 .....        24628        22725  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\000003AC_1.xwm
2024-05-13 23:26:08 .....        31330        30856  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\000003AD_1.xwm
2024-05-13 23:26:08 .....        15692        15070  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\000003AF_1.xwm
2024-05-13 23:26:08 .....        42500        41799  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\000003B1_1.xwm
2024-05-13 23:26:08 .....        71542        69914  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\000003B2_1.xwm
2024-05-13 23:26:08 .....        58138        56395  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\000003B4_1.xwm
2024-05-13 23:26:08 .....        49202        48321  Content\Core\Sound\Voice\Power Armor to the People.esp\PATTP_VT_Vendor_RarePowerArmor\000003B4_2.xwm
2023-01-02 21:43:44 .....         6334         4977  Content\Core\Sound\Voice\Power Armor to the People.esp\PlayerVoiceFemale01\00000393_1.fuz
2023-01-02 21:43:44 .....        13162        11110  Content\Core\Sound\Voice\Power Armor to the People.esp\PlayerVoiceFemale01\000003AA_1.fuz
2023-01-02 21:43:44 .....         7545         5879  Content\Core\Sound\Voice\Power Armor to the People.esp\PlayerVoiceMale01\00000393_1.fuz
2023-01-02 21:43:44 .....         9533         8100  Content\Core\Sound\Voice\Power Armor to the People.esp\PlayerVoiceMale01\000003AA_1.fuz
2024-06-03 02:58:10 D....            0            0  Content\Extensions\LLFP\
2024-06-03 02:58:10 D....            0            0  Content\Extensions\LLFP\F4SE\
2024-05-13 23:26:08 .....           36           34  Content\Extensions\LLFP\F4SE\plugins\LL_FourPlay.sample
2024-05-13 23:26:08 .....       344064       141727  Content\Extensions\LLFP\F4SE\plugins\LL_fourPlay_1_10_163.dll
2024-06-03 02:58:10 D....            0            0  Content\Extensions\LLFP\Scripts\Source\
2024-05-13 23:26:08 .....         4350         1746  Content\Extensions\LLFP\Scripts\LL_FourPlay.pex
2024-05-13 23:26:08 .....        15541         4800  Content\Extensions\LLFP\Scripts\Source\User\LL_FourPlay.psc
2021-12-02 13:02:08 .....        16738        16195  Content\Features\Power Armor to the People Abandonment - Nuka-World.esp
2021-12-02 13:03:56 .....        48028        33282  Content\Features\Power Armor to the People Abandonment.esp
2024-06-03 02:58:10 D....            0            0  Content\Patches\MogomraPAMs\
2021-11-29 22:35:58 .....          476          274  Content\Patches\Power Armor to the People - America Rising.esp
2024-05-01 15:43:22 .....        52520        39766  Content\Patches\Power Armor to the People - Automatron.esp
2022-01-21 22:05:40 .....        17507        15975  Content\Patches\Power Armor to the People - Brotherhood Power Armor Overhaul.esp
2022-01-24 22:05:14 .....         1275          474  Content\Patches\Power Armor to the People - Consistent Power Armor Overhaul.esp
2021-06-01 19:49:32 .....         3151          679  Content\Patches\Power Armor to the People - EMEncounters1.esp
2023-01-02 10:10:16 .....        31588        16349  Content\Patches\Power Armor to the People - Far Harbor.esp
2021-10-07 20:30:44 .....         5425         5240  Content\Patches\Power Armor to the People - Gunner Outfit Pack LL Integration.esp
2021-06-01 19:49:32 .....          796          319  Content\Patches\Power Armor to the People - Gunner Outfit Pack.esp
2022-01-24 22:05:14 .....         3828          631  Content\Patches\Power Armor to the People - Gunners vs Minutemen CC.esp
2021-10-02 15:01:28 .....       335421       302523  Content\Patches\Power Armor to the People - i73fi Scavengers.esp
2023-01-28 20:35:16 .....          941          801  Content\Patches\Power Armor to the People - Institute Power Armor - Corpus Praesidium.esp
2023-01-28 20:35:16 .....         2608         1141  Content\Patches\Power Armor to the People - Institute Power Armor - Institute Heavy Weaponry.esp
2022-01-24 22:05:14 .....          760          367  Content\Patches\Power Armor to the People - Minutemen Paint Job.esp
2024-05-13 23:26:08 .....        17924         2506  Content\Patches\Power Armor to the People - Next-Gen Update.esp
2023-03-25 20:13:18 .....        46893        19924  Content\Patches\Power Armor to the People - Nuka-World.esp
2024-04-29 21:10:54 .....        12077         2903  Content\Patches\Power Armor to the People - Overboss No Chains MPAM.esp
2023-03-25 20:15:34 .....        11906         2856  Content\Patches\Power Armor to the People - Overboss No Chains.esp
2024-05-09 01:00:38 .....          745          468  Content\Patches\Power Armor to the People - Power Armor Backpacks - T-60 Equipment.esp
2024-05-09 20:28:04 .....         1295          377  Content\Patches\Power Armor to the People - Power Armor Backpacks.esp
2024-05-04 23:22:54 .....         1621          498  Content\Patches\Power Armor to the People - Power Armor Paints.esp
2024-05-09 20:16:32 .....          936          376  Content\Patches\Power Armor to the People - Power Armor Pouches.esp
2023-03-25 20:28:06 .....        23817         2903  Content\Patches\Power Armor to the People - R88 Simple Sorter.esp
2021-10-15 12:29:34 .....        14936         2765  Content\Patches\Power Armor to the People - Raider Chop Shop.esp
2022-01-30 14:44:42 .....        24085        19718  Content\Patches\Power Armor to the People - Raider Overhaul.esp
2022-12-23 10:45:46 .....         2667          825  Content\Patches\Power Armor to the People - Stuff of Legend - Far Harbor.esp
2024-05-03 19:47:20 .....        57324         8319  Content\Patches\Power Armor to the People - Stuff of Legend.esp
2024-06-03 02:52:32 .....         7508         1173  Content\Patches\Power Armor to the People - SuperAlloys Cohesion.esp
2024-05-09 21:44:02 .....         2762         1082  Content\Patches\Power Armor to the People - T-60 Equipment.esp
2022-12-29 22:58:32 .....        33773        16499  Content\Patches\Power Armor to the People - Vault 111 Settlement.esp
2024-04-30 20:53:16 .....         1262          520  Content\Patches\Power Armor to the People - Visual Tesla Coils.esp
2021-09-19 22:58:10 .....         4684         4267  Content\Patches\Power Armor to the People - We Are The Minutemen.esp
2024-05-13 23:26:08 .....         5393         1478  Content\Patches\MogomraPAMs\1.4\Power Armor to the People - MogomraPAMs.esp
2023-01-26 06:03:58 .....         6117         1638  Content\Patches\MogomraPAMs\2.0\Power Armor to the People - MogomraPAMs.esp
2024-05-09 11:51:38 .....       133229        14043  Content\Patches\Patch Variants\Power Armor to the People - AWKCR-SAR.esp
2024-05-09 11:51:38 .....       119067        12720  Content\Patches\Patch Variants\Power Armor to the People - AWKCR.esp
2024-05-09 11:51:38 .....        68407         8307  Content\Patches\Patch Variants\Power Armor to the People - Some Assembly Required.esp
2024-05-08 00:27:56 .....        20755         2902  Content\Patches\X-01 Tesla Upgrade Kit\Power Armor to the People - X-01 Tesla Upgrade Kit.esp
2024-05-07 20:25:12 .....        35579         4747  Content\Patches\X-01 Tesla Upgrade Kit\Patch Variants\Power Armor to the People - X-01 Tesla Upgrade Kit - AWKCR-SAR.esp
2024-05-07 20:25:12 .....        35550         4707  Content\Patches\X-01 Tesla Upgrade Kit\Patch Variants\Power Armor to the People - X-01 Tesla Upgrade Kit - AWKCR.esp
2024-05-07 20:25:12 .....        16186         2448  Content\Patches\X-01 Tesla Upgrade Kit\Patch Variants\Power Armor to the People - X-01 Tesla Upgrade Kit - CPAO-SAR.esp
2023-01-28 21:18:26 .....        11112         2109  Content\Patches\X-01 Tesla Upgrade Kit\Patch Variants\Power Armor to the People - X-01 Tesla Upgrade Kit - CPAO.esp
2024-05-07 20:25:12 .....        22622         2781  Content\Patches\X-01 Tesla Upgrade Kit\Patch Variants\Power Armor to the People - X-01 Tesla Upgrade Kit - SAR.esp
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Classic Advanced Power Armor\
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Combat Power Armor\
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Hellcat Power Armor\
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Midwest Power Armor Revolution\
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Red Shift Power Armor\
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\SE-01 Power Armor\
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\T-65 Power Armor\
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Ultracite Power Armor\
2023-03-22 20:25:48 .....        10576         2719  Content\Power Armor Sets\Cagebreaker Power Armor\Power Armor to the People - Cagebreaker Power Armor.esp
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Captain Cosmos (Creation Club)\Scripts\
2023-03-25 20:12:52 .....        23904        11654  Content\Power Armor Sets\Captain Cosmos (Creation Club)\Power Armor to the People - Captain Cosmos.esp
2024-06-03 02:58:10 D....            0            0  Content\Power Armor Sets\Captain Cosmos (Creation Club)\Scripts\Fragments\
2021-06-01 19:49:32 .....         2468         1038  Content\Power Armor Sets\Captain Cosmos (Creation Club)\Scripts\Fragments\Terminals\TERM_ccSWKFO4001_Terminal_Re_01000C4F.pex
2024-05-04 00:32:42 .....        63273        24181  Content\Power Armor Sets\Classic Advanced Power Armor\ESL Version\Power Armor to the People - Classic Advanced Power Armor.esp
2024-05-13 23:26:10 .....        63273        24180  Content\Power Armor Sets\Classic Advanced Power Armor\ESP Version\Power Armor to the People - Classic Advanced Power Armor.esp
2024-05-04 00:49:28 .....        61853        23886  Content\Power Armor Sets\Classic Advanced Power Armor\Overhaul\Power Armor to the People - Classic Advanced Power Armor.esp
2024-05-03 20:37:50 .....        21206         4627  Content\Power Armor Sets\Combat Power Armor\Nighthawk\Power Armor to the People - Combat Power Armor.esp
2024-05-03 21:24:04 .....        20014         4376  Content\Power Armor Sets\Combat Power Armor\Original\Power Armor to the People - Combat Power Armor.esp
2023-03-25 20:14:44 .....        21757         5413  Content\Power Armor Sets\Enclave Power Armor\Power Armor to the People - Enclave Power Armor.esp
2023-03-25 20:17:12 .....        17614         4803  Content\Power Armor Sets\Enclave X-02 Black Devil Power Armor\Power Armor to the People - Enclave X-02 (Black Devil).esp
2023-03-25 20:11:20 .....        23731         4641  Content\Power Armor Sets\Enclave X-02 Power Armor\Power Armor to the People - Enclave X-02 - All Factions Paintjob.esp
2023-03-25 20:11:20 .....        30569         5772  Content\Power Armor Sets\Enclave X-02 Power Armor\Power Armor to the People - Enclave X-02.esp
2022-12-29 01:01:32 .....        15503         2235  Content\Power Armor Sets\Enclave X-02 Power Armor\Patch Variants\Power Armor to the People - Enclave X-02 - SAR-All Factions Paintjob.esp
2022-12-29 01:01:32 .....        13160         1740  Content\Power Armor Sets\Enclave X-02 Power Armor\Patch Variants\Power Armor to the People - Enclave X-02 - SAR.esp
2023-03-25 20:12:52 .....        33469         6157  Content\Power Armor Sets\Excavator Power Armor\Power Armor to the People - Excavator Power Armor.esp
2024-05-13 23:26:12 .....        30596        22610  Content\Power Armor Sets\Hellcat Power Armor\1.1\Power Armor to the People - Hellcat Power Armor.esp
2023-03-25 20:17:12 .....        44187        19254  Content\Power Armor Sets\Hellcat Power Armor\1.2\Power Armor to the People - Hellcat Power Armor.esp
2022-12-04 01:07:46 .....        10422         2253  Content\Power Armor Sets\Hellfire Power Armor (Creation Club)\Power Armor to the People - Hellfire CC.esp
2023-03-25 20:11:00 .....        20416         4035  Content\Power Armor Sets\Hellfire X-03 Power Armor\Power Armor to the People - Hellfire X-03 - All Factions Paintjob.esp
2023-03-25 20:17:40 .....        27311         5210  Content\Power Armor Sets\Hellfire X-03 Power Armor\Power Armor to the People - Hellfire X-03.esp
2021-05-08 21:59:02 .....        12251         1684  Content\Power Armor Sets\Hellfire X-03 Power Armor\Patch Variants\Power Armor to the People - Hellfire X-03 - SAR-All Factions Paintjob.esp
2021-05-08 21:59:02 .....        11597         1443  Content\Power Armor Sets\Hellfire X-03 Power Armor\Patch Variants\Power Armor to the People - Hellfire X-03 - SAR.esp
2023-01-01 01:01:48 .....         4586         1035  Content\Power Armor Sets\Horse Power Armor (Creation Club)\Power Armor to the People - Horse Power Armor.esp
2021-06-07 07:35:30 .....         5487         2215  Content\Power Armor Sets\Institute Power Armor\Power Armor to the People - Institute Power Armor - SAR.esp
2023-03-25 20:12:10 .....        62593        41134  Content\Power Armor Sets\Institute Power Armor\Power Armor to the People - Institute Power Armor.esp
2023-03-25 20:17:12 .....        29939        23121  Content\Power Armor Sets\Liberty Power Armor\Power Armor to the People - Liberty Power Armor.esp
2023-03-25 20:15:34 .....        54759        23091  Content\Power Armor Sets\Midwest Power Armor Evolution\Power Armor to the People - Midwest Power Armor Evolution.esp
2024-05-02 23:52:32 .....        49143        18900  Content\Power Armor Sets\Midwest Power Armor Revolution\ESL Version\Power Armor to the People - Midwest Power Armor Revolution.esp
2024-05-13 23:26:12 .....        49135        18900  Content\Power Armor Sets\Midwest Power Armor Revolution\ESP Version\Power Armor to the People - Midwest Power Armor Revolution.esp
2023-03-25 20:17:12 .....        59869        32526  Content\Power Armor Sets\Red Shift Power Armor\ESL Version\Power Armor to the People - Red Shift Power Armor.esp
2024-05-13 23:26:12 .....        59869        32524  Content\Power Armor Sets\Red Shift Power Armor\ESP Version\Power Armor to the People - Red Shift Power Armor.esp
2024-04-27 12:38:40 .....        21217         5374  Content\Power Armor Sets\SE-01 Power Armor\ESL Version\Power Armor to the People - Select Power Armor.esp
2024-05-13 23:26:12 .....        20962         5289  Content\Power Armor Sets\SE-01 Power Armor\ESP Version\Power Armor to the People - Select Power Armor.esp
2024-06-03 02:58:12 D....            0            0  Content\Power Armor Sets\Settler Vigilante Power Armor\Scripts\
2024-04-22 00:32:00 .....         1589          690  Content\Power Armor Sets\Settler Vigilante Power Armor\Power Armor to the People - Settler Vigilante Power Armor - America Rising 2.esp
2021-10-29 11:47:24 .....         1468          639  Content\Power Armor Sets\Settler Vigilante Power Armor\Power Armor to the People - Settler Vigilante Power Armor - America Rising.esp
2023-03-25 20:15:34 .....        28677         7506  Content\Power Armor Sets\Settler Vigilante Power Armor\Power Armor to the People - Settler Vigilante Power Armor.esp"#;


    let output = ListOutput::from_str(EXAMPLE_3).unwrap().entries;

    let assert_contains = |path| {
        assert!(output.iter().map(|e| &e.name).any(|name| name == &PathBuf::from(path)), "does not contains [{path}]")
    };
    
    assert_contains("Content/Patches/Power Armor to the People - Automatron.esp");
    Ok(())
}

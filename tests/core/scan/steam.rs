use dlss_studio::core::scan::steam::*;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_discover_steam_runs_safely() {
    let games = discover_steam();
    for game in games {
        assert_eq!(game.launcher, "Steam");
        assert!(!game.name.is_empty());
        assert!(game.exe_path.exists());
    }
}

#[test]
fn test_discover_steam_from_path_mock() {
    let temp = TempDir::new("steam_mock");
    let steam_root = temp.path();

    // Create steamapps and libraryfolders.vdf
    let apps_dir = steam_root.join("steamapps");
    fs::create_dir_all(&apps_dir).unwrap();

    let vdf_content = format!(
        r#""libraryfolders"
{{
    "0"
    {{
        "path" "{}"
        "label" ""
    }}
}}"#,
        steam_root.to_string_lossy().replace('\\', "\\\\")
    );
    fs::write(apps_dir.join("libraryfolders.vdf"), vdf_content).unwrap();

    // Create mock game folder and exe
    let game_dir = apps_dir.join("common").join("MockSteamGame");
    fs::create_dir_all(&game_dir).unwrap();
    fs::write(game_dir.join("game.exe"), b"MZ dummy PE").unwrap();

    // Create appmanifest_99999.acf
    let acf_content = r#""AppState"
{
    "appid" "99999"
    "name" "Mock Steam Game Title"
    "installdir" "MockSteamGame"
}"#;
    fs::write(apps_dir.join("appmanifest_99999.acf"), acf_content).unwrap();

    let games = discover_steam_from_path(steam_root);
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].name, "Mock Steam Game Title");
    assert_eq!(games[0].launcher, "Steam");
}

#[test]
fn test_discover_steam_multi_library_and_poster_and_filter() {
    let temp = TempDir::new("steam_multi_lib");
    let steam_root = temp.path().join("SteamRoot");
    let second_lib = temp.path().join("SecondLibrary");

    let apps_dir1 = steam_root.join("steamapps");
    let apps_dir2 = second_lib.join("steamapps");
    fs::create_dir_all(&apps_dir1).unwrap();
    fs::create_dir_all(&apps_dir2).unwrap();

    // libraryfolders.vdf pointing to both
    let vdf_content = format!(
        r#""libraryfolders"
{{
    "0"
    {{
        "path" "{}"
    }}
    "1"
    {{
        "path" "{}"
    }}
}}"#,
        steam_root.to_string_lossy().replace('\\', "\\\\"),
        second_lib.to_string_lossy().replace('\\', "\\\\")
    );
    fs::write(apps_dir1.join("libraryfolders.vdf"), vdf_content).unwrap();

    // Game 1 in lib1 with poster
    let game1_dir = apps_dir1.join("common").join("Game1");
    fs::create_dir_all(&game1_dir).unwrap();
    fs::write(game1_dir.join("game1.exe"), b"MZ dummy PE").unwrap();

    let poster_dir = steam_root.join("appcache").join("librarycache").join("1001");
    fs::create_dir_all(&poster_dir).unwrap();
    fs::write(poster_dir.join("library_600x900.jpg"), b"JPEG_DATA").unwrap();

    let acf1 = r#""AppState" { "appid" "1001" "name" "Genuine Game 1" "installdir" "Game1" }"#;
    fs::write(apps_dir1.join("appmanifest_1001.acf"), acf1).unwrap();

    // Game 2 in lib2
    let game2_dir = apps_dir2.join("common").join("Game2");
    fs::create_dir_all(&game2_dir).unwrap();
    fs::write(game2_dir.join("game2.exe"), b"MZ dummy PE").unwrap();

    let acf2 = r#""AppState" { "appid" "1002" "name" "Genuine Game 2" "installdir" "Game2" }"#;
    fs::write(apps_dir2.join("appmanifest_1002.acf"), acf2).unwrap();

    // Non-game title (e.g. Proton / Soundtrack) that should be filtered out
    let tool_dir = apps_dir1.join("common").join("ProtonTool");
    fs::create_dir_all(&tool_dir).unwrap();
    fs::write(tool_dir.join("tool.exe"), b"MZ dummy PE").unwrap();
    let acf_tool = r#""AppState" { "appid" "9999" "name" "Proton Experimental" "installdir" "ProtonTool" }"#;
    fs::write(apps_dir1.join("appmanifest_9999.acf"), acf_tool).unwrap();

    // Corrupted ACF (missing installdir)
    fs::write(apps_dir1.join("appmanifest_bad.acf"), r#""AppState" { "appid" "8888" }"#).unwrap();

    let games = discover_steam_from_path(&steam_root);
    assert_eq!(games.len(), 2, "Should discover exactly 2 genuine games and exclude tools/corrupted manifests");
    let names: Vec<String> = games.iter().map(|g| g.name.clone()).collect();
    assert!(names.contains(&"Genuine Game 1".to_string()));
    assert!(names.contains(&"Genuine Game 2".to_string()));

    let g1 = games.iter().find(|g| g.name == "Genuine Game 1").unwrap();
    assert!(g1.poster.is_some(), "Poster must be resolved from librarycache");
}

#[test]
fn test_discover_steam_missing_vdf_fallback() {
    let temp = TempDir::new("steam_no_vdf");
    let steam_root = temp.path();
    let apps_dir = steam_root.join("steamapps");
    fs::create_dir_all(&apps_dir).unwrap();

    let game_dir = apps_dir.join("common").join("FallbackGame");
    fs::create_dir_all(&game_dir).unwrap();
    fs::write(game_dir.join("fb.exe"), b"MZ dummy PE").unwrap();

    fs::write(
        apps_dir.join("appmanifest_777.acf"),
        r#""AppState" { "appid" "777" "name" "Fallback Game" "installdir" "FallbackGame" }"#,
    ).unwrap();

    let games = discover_steam_from_path(steam_root);
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].name, "Fallback Game");
}


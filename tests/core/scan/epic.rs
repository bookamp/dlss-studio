use dlss_studio::core::scan::epic::*;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_discover_epic_runs_safely() {
    let games = discover_epic();
    for game in games {
        assert_eq!(game.launcher, "Epic Games");
        assert!(!game.name.is_empty());
    }
}

#[test]
fn test_discover_epic_from_manifests_mock() {
    let temp = TempDir::new("epic_manifests");
    let manifests_dir = temp.join("Manifests");
    fs::create_dir_all(&manifests_dir).unwrap();

    // Create a mock game directory with an exe
    let game_dir = temp.join("MockGame");
    fs::create_dir_all(&game_dir).unwrap();
    fs::write(game_dir.join("game.exe"), b"MZ dummy PE executable").unwrap();

    let manifest_content = serde_json::json!({
        "InstallLocation": game_dir.to_str().unwrap(),
        "DisplayName": "Mock Epic Title"
    });

    fs::write(
        manifests_dir.join("ABC12345.item"),
        serde_json::to_string(&manifest_content).unwrap(),
    ).unwrap();

    // Also write a non-.item file to test filtering
    fs::write(manifests_dir.join("ignore_me.txt"), b"ignore").unwrap();

    let games = discover_epic_from_manifests(&manifests_dir);
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].name, "Mock Epic Title");
    assert_eq!(games[0].launcher, "Epic Games");
}

#[test]
fn test_discover_epic_edge_cases() {
    let temp = TempDir::new("epic_edges");
    let manifests_dir = temp.join("Manifests");
    fs::create_dir_all(&manifests_dir).unwrap();

    // 1. Nonexistent directory returns empty
    let non_existent = temp.join("DoesntExist");
    assert!(discover_epic_from_manifests(&non_existent).is_empty());

    // 2. Corrupted JSON file
    fs::write(manifests_dir.join("corrupt.item"), b"{ not valid json").unwrap();

    // 3. Missing InstallLocation
    fs::write(manifests_dir.join("missing_loc.item"), serde_json::json!({
        "DisplayName": "Missing Loc Game"
    }).to_string()).unwrap();

    // 4. InstallLocation does not exist on disk
    fs::write(manifests_dir.join("ghost.item"), serde_json::json!({
        "InstallLocation": temp.join("GhostGame").to_str().unwrap(),
        "DisplayName": "Ghost Game"
    }).to_string()).unwrap();

    // 5. Valid game without DisplayName
    let no_title_dir = temp.join("NoTitleGame");
    fs::create_dir_all(&no_title_dir).unwrap();
    fs::write(no_title_dir.join("game.exe"), b"MZ dummy PE").unwrap();
    fs::write(manifests_dir.join("no_title.item"), serde_json::json!({
        "InstallLocation": no_title_dir.to_str().unwrap()
    }).to_string()).unwrap();

    // 6. Duplicate manifest pointing to same location
    fs::write(manifests_dir.join("dup.item"), serde_json::json!({
        "InstallLocation": no_title_dir.to_str().unwrap(),
        "DisplayName": "Duplicate Game Title"
    }).to_string()).unwrap();

    let games = discover_epic_from_manifests(&manifests_dir);
    assert_eq!(games.len(), 1, "Only the genuine existing game should be returned, duplicate skipped");
    assert_eq!(games[0].launcher, "Epic Games");
}


use dlss_studio::core::state::model::*;
use dlss_studio::core::state::store::*;
use dlss_studio::core::scan::GameEntry;

#[test]
fn test_hidden_games_persistence_and_normalization() {
    let mut state = AppState::default();
    let game_dir = std::path::PathBuf::from(r"E:\GOG Games\Being a DIK");
    let game = GameEntry {
        name: "Being a DIK".to_string(),
        dir: game_dir.clone(),
        exe_path: game_dir.join("Being a DIK.exe"),
        exe_rel: "Being a DIK.exe".to_string(),
        bitness: 64,
        api: "DirectX 11".to_string(),
        dlss_version: None,
        has_frame_generation: false,
        can_inject_fg: false,
        optiscaler_installed: false,
        optiscaler_presr: false,
        optiscaler_passes: 1,
        mfg_unlock_installed: false,
        has_backup: false,
        launcher: "GOG".to_string(),
        poster: None,
        reshade_installed: false,
        reshade_version: None,
        reshade_addon_support: false,
        addon_installed: false,
        installed_route: None,
        files: Vec::new(),
        available_exes: Vec::new(),
        is_laa: true,
        nr_style: 0,
        nr_style_enabled: false,
        mfg_multiplier: 4,
        has_anti_cheat: false,
    };
    state.cached_games.push(game.clone());
    assert!(!state.is_hidden(&game_dir));

    state.hide_game(&game_dir);
    assert!(state.is_hidden(&game_dir));
    // Test case and slash insensitivity
    assert!(state.is_hidden(std::path::Path::new("e:/gog games/being a dik/")));
    // Cached games should no longer contain it
    assert_eq!(state.cached_games.len(), 0);

    // Unhide
    state.unhide_game(&game_dir);
    assert!(!state.is_hidden(&game_dir));
}

#[test]
fn test_base_addons_mandatory_and_custom_addons() {
    let mut state = AppState::default();
    // Base add-ons are always active
    assert!(is_addon_active(&state, "builtin:renodx"));
    assert!(is_addon_active(&state, "builtin:mfgunlock"));
    assert!(is_addon_active(&state, "builtin:feeder"));
    assert!(!is_addon_active(&state, "builtin:overlay"), "Overlay must be inactive");

    // Attempting to toggle off a base add-on is ignored
    toggle_addon_in_state(&mut state, "builtin:renodx", false);
    assert!(is_addon_active(&state, "builtin:renodx"), "Base add-ons cannot be deactivated");

    // Custom add-ons can be toggled
    let custom_id = "C:\\Mods\\reshade\\my_addon.addon64";
    assert!(!is_addon_active(&state, custom_id));
    toggle_addon_in_state(&mut state, custom_id, true);
    assert!(is_addon_active(&state, custom_id));
    toggle_addon_in_state(&mut state, custom_id, false);
    assert!(!is_addon_active(&state, custom_id));
}

#[test]
fn test_library_json_version_migration() {
    // Legacy library.json lacking "version" field
    let legacy_json = r#"{
        "theme": "dark",
        "lang": "en",
        "cachedGames": [
            {
                "name": "Cyberpunk 2077",
                "dir": "C:/Games/Cyberpunk 2077",
                "exe_path": "C:/Games/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe",
                "exe_rel": "bin/x64/Cyberpunk2077.exe",
                "bitness": 64,
                "api": "DirectX 12",
                "has_frame_generation": false,
                "optiscaler_installed": false,
                "optiscaler_presr": false,
                "optiscaler_passes": 1,
                "mfg_unlock_installed": false,
                "has_backup": false,
                "launcher": "Steam",
                "reshade_installed": false,
                "reshade_addon_support": false,
                "addon_installed": false,
                "files": [],
                "available_exes": [
                    { "name": "Main", "path": "C:/Games/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe", "rel": "bin/x64/Cyberpunk2077.exe", "api": "DirectX 12", "bitness": 64 }
                ]
            }
        ],
        "folders": ["D:/MyGames/"]
    }"#;

    let mut state: AppState = serde_json::from_str(legacy_json).expect("deserialize legacy json");
    assert_eq!(state.version, None, "Legacy state must parse with version: None");

    let from_v = state.version.clone();
    let migrated = migrate_app_state(&mut state, from_v.as_deref(), CURRENT_APP_VERSION);
    assert!(migrated, "migrate_app_state must report changes");
    assert_eq!(state.version.as_deref(), Some(CURRENT_APP_VERSION));

    // Verify path normalization occurred
    assert_eq!(state.cached_games[0].dir, std::path::PathBuf::from("C:\\Games\\Cyberpunk 2077"));
    assert_eq!(state.cached_games[0].exe_path, std::path::PathBuf::from("C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe"));
    assert_eq!(state.cached_games[0].available_exes[0].path, std::path::PathBuf::from("C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe"));
    assert_eq!(state.folders[0], "D:\\MyGames");

    // Verify mandatory add-ons were populated
    assert!(state.addons.contains(&"builtin:renodx".to_string()));
    assert!(state.addons.contains(&"builtin:mfgunlock".to_string()));
    assert!(state.addons.contains(&"builtin:feeder".to_string()));

    // Second migration with matching version must be no-op
    let from_v2 = state.version.clone();
    let second_migrated = migrate_app_state(&mut state, from_v2.as_deref(), CURRENT_APP_VERSION);
    assert!(!second_migrated, "Second migration on up-to-date state must be no-op");
}

#[test]
fn test_touch_and_recents_lifecycle() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let test_dir = "C:\\Games\\TouchTestGame";
    touch(test_dir);
    let s = load_state();
    assert!(s.recents.iter().any(|r| r.dir == test_dir));

    // Touch again to test deduplication and moving to front
    touch(test_dir);
    let s2 = load_state();
    assert_eq!(s2.recents.first().map(|r| r.dir.as_str()), Some(test_dir));
}

#[test]
fn test_sync_overlay_preferences_execution() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut state = load_state();
    state.overlay_theme = "azure".to_string();
    state.overlay_hotkey = "Ctrl+Shift+O".to_string();
    let _ = save_state(&state);

    sync_overlay_preferences(&state);
}

#[test]
fn test_add_and_remove_custom_addon_lifecycle() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut state = load_state();
    let initial_count = state.addon_files.len();

    let addon = AddonFileEntry {
        path: "C:\\Mods\\test.addon64".to_string(),
        name: Some("Test Addon".to_string()),
        description: Some("A test addon".to_string()),
        tag: Some("Shader".to_string()),
    };

    add_custom_addon(&mut state, addon);
    assert_eq!(state.addon_files.len(), initial_count + 1);

    remove_custom_addon(&mut state, "C:\\Mods\\test.addon64");
    assert_eq!(state.addon_files.len(), initial_count);
}

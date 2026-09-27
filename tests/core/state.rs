use dlss_studio::core::state::{
    ago_localized, clean_path_separators, is_addon_active, is_path_protected, migrate_app_state,
    resolve_appdata_dir_internal, toggle_addon_in_state, AppState, CURRENT_APP_VERSION,
};
use dlss_studio::core::scan::GameEntry;

#[test]
fn test_rust_theme_default_and_serde() {
    let default_state = AppState::default();
    assert!(default_state.rust_theme, "Rust theme should be enabled by default");

    let json = serde_json::to_string(&default_state).unwrap();
    assert!(json.contains(r#""rustTheme":true"#));

    // Test missing key defaults to true
    let partial_json = r#"{"folders":[],"theme":"dark"}"#;
    let parsed: AppState = serde_json::from_str(partial_json).unwrap();
    assert!(parsed.rust_theme, "Missing rustTheme in JSON must default to true");

    // Test explicit false is preserved
    let false_json = r#"{"folders":[],"rustTheme":false}"#;
    let parsed_false: AppState = serde_json::from_str(false_json).unwrap();
    assert!(!parsed_false.rust_theme, "Explicit false rustTheme must be preserved");
}

#[test]
fn test_run_in_background_serde() {
    let default_state = AppState::default();
    assert!(default_state.run_in_background, "Run in background should default to true");

    let json = serde_json::to_string(&default_state).unwrap();
    assert!(json.contains(r#""runInBackground":true"#));

    let partial = r#"{"folders":[],"theme":"dark"}"#;
    let parsed: AppState = serde_json::from_str(partial).unwrap();
    assert!(parsed.run_in_background, "Missing runInBackground must default to true");

    let false_json = r#"{"runInBackground":false}"#;
    let parsed_false: AppState = serde_json::from_str(false_json).unwrap();
    assert!(!parsed_false.run_in_background, "Explicit false runInBackground must be preserved");
}

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
fn test_custom_names_persistence() {
    let mut state = AppState::default();
    let game_dir = std::path::Path::new(r"E:\Games\CustomGame");

    // Initially none
    assert_eq!(state.get_custom_name(game_dir), None);

    // Set custom name
    state.set_custom_name(game_dir, "My Custom Game Title");
    assert_eq!(state.get_custom_name(game_dir), Some(&"My Custom Game Title".to_string()));

    // Check path normalization (forward slashes and trailing slash)
    let alt_dir = std::path::Path::new("e:/games/customgame/");
    assert_eq!(state.get_custom_name(alt_dir), Some(&"My Custom Game Title".to_string()));

    // Test serialization and deserialization
    let json = serde_json::to_string(&state).expect("serialize");
    let loaded: AppState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(loaded.get_custom_name(game_dir), Some(&"My Custom Game Title".to_string()));

    // Empty string resets / removes
    state.set_custom_name(game_dir, "   ");
    assert_eq!(state.get_custom_name(game_dir), None);

    state.set_custom_name(game_dir, "Temporary");
    state.set_custom_name(game_dir, "");
    assert_eq!(state.get_custom_name(game_dir), None);
}

#[test]
fn test_fresh_state_defaults() {
    let state = AppState::default();
    assert!(state.run_in_background);
    assert!(state.rust_theme);
    assert!(!state.auto_scan_drives);
    assert!(state.cached_games.is_empty());
}

#[test]
fn test_is_path_protected_detection() {
    assert!(is_path_protected(r"C:\Program Files\DLSS 5 Studio"));
    assert!(is_path_protected(r"C:\Program Files (x86)\DLSS 5 Studio"));
    assert!(is_path_protected("C:/Program Files/DLSS 5 Studio"));
    assert!(is_path_protected(r"C:\Windows\System32"));
    assert!(is_path_protected(r"C:\Program Files\WindowsApps\Game"));

    // Unprotected directories
    assert!(!is_path_protected(r"D:\Games\DLSS 5 Studio"));
    assert!(!is_path_protected(r"C:\Games\DLSS 5 Studio"));
    assert!(!is_path_protected(r"E:\Tools\ModManager"));
}

#[test]
fn test_resolve_appdata_dir_portable_priority() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio-portable.exe",
        Some(std::path::Path::new(r"D:\Games\DLSS 5 Studio")),
        Some(r#"{"data_dir": "E:\\CustomStorage"}"#),
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"C:\Users\Tester\AppData\Roaming\dlss-5-studio"));
}

#[test]
fn test_resolve_appdata_dir_storage_json_override() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio.exe",
        Some(std::path::Path::new(r"C:\Program Files\DLSS 5 Studio")),
        Some(r#"{"data_dir": "D:\\MyCustomStorage"}"#),
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"D:\MyCustomStorage"));
}

#[test]
fn test_resolve_appdata_dir_unprotected_install_defaults_to_data() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio.exe",
        Some(std::path::Path::new(r"D:\Games\DLSS 5 Studio")),
        None,
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"D:\Games\DLSS 5 Studio\data"));
}

#[test]
fn test_resolve_appdata_dir_protected_install_defaults_to_programdata() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio.exe",
        Some(std::path::Path::new(r"C:\Program Files\DLSS 5 Studio")),
        None,
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"C:\ProgramData\dlss-5-studio"));
}

#[test]
fn test_ago_localized_multilingual() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

    // 30 seconds ago
    let t_just_now = now_ms.saturating_sub(30 * 1000);
    assert_eq!(ago_localized("en", t_just_now), "just now");
    assert_eq!(ago_localized("ru", t_just_now), "только что");
    assert_eq!(ago_localized("de", t_just_now), "gerade eben");
    assert_eq!(ago_localized("zh", t_just_now), "刚刚");

    // 10 minutes ago
    let t_10m = now_ms.saturating_sub(10 * 60 * 1000);
    assert_eq!(ago_localized("en", t_10m), "10m ago");
    assert_eq!(ago_localized("ru", t_10m), "10 мин назад");
    assert_eq!(ago_localized("de", t_10m), "vor 10 Min.");
    assert_eq!(ago_localized("zh", t_10m), "10分钟前");

    // 2 hours ago
    let t_2h = now_ms.saturating_sub(2 * 3600 * 1000);
    assert_eq!(ago_localized("en", t_2h), "2h ago");
    assert_eq!(ago_localized("ru", t_2h), "2 ч назад");
    assert_eq!(ago_localized("de", t_2h), "vor 2 Std.");
    assert_eq!(ago_localized("zh", t_2h), "2小时前");

    // 1 day ago (yesterday)
    let t_yest = now_ms.saturating_sub(25 * 3600 * 1000);
    assert_eq!(ago_localized("en", t_yest), "yesterday");
    assert_eq!(ago_localized("ru", t_yest), "вчера");
    assert_eq!(ago_localized("de", t_yest), "gestern");
    assert_eq!(ago_localized("zh", t_yest), "昨天");

    // 3 days ago
    let t_3d = now_ms.saturating_sub(3 * 86400 * 1000);
    assert_eq!(ago_localized("en", t_3d), "3d ago");
    assert_eq!(ago_localized("ru", t_3d), "3 дн назад");
    assert_eq!(ago_localized("de", t_3d), "vor 3 Tagen");
    assert_eq!(ago_localized("zh", t_3d), "3天前");
}

#[test]
fn test_language_persists_across_serialization() {
    let default_state = AppState::default();
    assert!(!default_state.lang.is_empty(), "Default language must not be empty");

    // Explicitly set language
    let mut custom_state = AppState::default();
    custom_state.lang = "de".to_string();

    let json = serde_json::to_string(&custom_state).expect("serialize state");
    let restored: AppState = serde_json::from_str(&json).expect("deserialize state");
    assert_eq!(restored.lang, "de", "Explicit user language selection must persist across serialization");
}

#[test]
fn test_clean_path_separators_mixed_slashes_and_drive_casing() {
    let raw = std::path::Path::new("d:/program files (x86)/steam\\steamapps\\common\\left 4 dead");
    let cleaned = clean_path_separators(raw);
    assert_eq!(
        cleaned,
        std::path::PathBuf::from(r"D:\program files (x86)\steam\steamapps\common\left 4 dead")
    );

    let trailing = std::path::Path::new("c:/games/test/");
    assert_eq!(clean_path_separators(trailing), std::path::PathBuf::from(r"C:\games\test"));
}

#[test]
fn test_clean_path_separators_edge_cases() {
    let rel = std::path::Path::new("games/steam/left 4 dead");
    assert_eq!(clean_path_separators(rel), std::path::PathBuf::from(r"games\steam\left 4 dead"));

    let empty = std::path::Path::new("");
    assert_eq!(clean_path_separators(empty), std::path::PathBuf::from(""));

    let drive_only = std::path::Path::new("e:/");
    assert_eq!(clean_path_separators(drive_only), std::path::PathBuf::from("E:"));

    let unc = std::path::Path::new(r"\\server\share/games\steam");
    assert_eq!(clean_path_separators(unc), std::path::PathBuf::from(r"\\server\share\games\steam"));
}

#[test]
fn test_vibepollo_auto_add_default_and_serde() {
    let default_state = AppState::default();
    assert!(!default_state.vibepollo_auto_add, "Vibepollo auto add must strictly default to false");

    let empty_json = "{}";
    let parsed: AppState = serde_json::from_str(empty_json).expect("deserialize empty json");
    assert!(!parsed.vibepollo_auto_add, "Missing vibepolloAutoAdd field must default to false");

    let with_true = r#"{"vibepolloAutoAdd": true}"#;
    let parsed_true: AppState = serde_json::from_str(with_true).expect("deserialize true");
    assert!(parsed_true.vibepollo_auto_add);

    let mut s = AppState::default();
    s.vibepollo_auto_add = true;
    let ser = serde_json::to_string(&s).expect("serialize");
    assert!(ser.contains("\"vibepolloAutoAdd\":true"));
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

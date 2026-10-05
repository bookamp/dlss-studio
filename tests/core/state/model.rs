use dlss_studio::core::state::model::*;

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

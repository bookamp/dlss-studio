use dlss_studio::core::scan::gog::*;
use dlss_studio::core::scan::scan_game_directory;
use std::path::Path;

#[test]
fn test_discover_gog_runs_safely() {
    let games = discover_gog();
    for game in games {
        assert_eq!(game.launcher, "GOG");
        assert!(!game.name.is_empty());
    }
}

#[test]
fn test_control_gog_metadata_and_art_detection() {
    let control_dir = Path::new(r"D:\Games\GoG\Control");
    if control_dir.is_dir() {
        let (gog_name, gog_poster, gog_id) = extract_gog_metadata(control_dir);
        assert_eq!(gog_name.as_deref(), Some("Control Ultimate Edition"));
        assert_eq!(gog_id.as_deref(), Some("2049187585"));
        assert!(gog_poster.is_some(), "GOG Galaxy local vertical cover must be discovered");
        assert!(gog_poster.unwrap().contains("http://dlss-art.localhost/art/"));

        let game = scan_game_directory(control_dir).expect("Control directory must be scanned");
        assert_eq!(game.launcher, "GOG");
        assert_eq!(game.name, "Control Ultimate Edition");
        assert!(game.poster.is_some(), "Game poster must be populated with discovered cover");
    }
}

#[test]
fn test_hermetic_gog_metadata_and_local_cover_extraction() {
    use crate::common::TempDir;

    let temp = TempDir::new("gog_hermetic");
    
    // 1. Create goggame-12345.info
    let info_path = temp.join("goggame-12345.info");
    let _ = std::fs::write(&info_path, r#"{"gameId": "12345", "name": "Hermetic GOG Title"}"#);

    // 2. Stage webcache under ProgramData if accessible or test fallback
    let (name, poster, gid) = extract_gog_metadata(temp.path());
    assert_eq!(name.as_deref(), Some("Hermetic GOG Title"));
    assert_eq!(gid.as_deref(), Some("12345"));
    let _ = poster;

    // 3. Test find_local_gog_cover with invalid ID
    let missing_cover = find_local_gog_cover("999999999999");
    assert_eq!(missing_cover, None);
}

#[test]
fn test_gog_local_cover_cache_variants_and_numeric_id() {
    use crate::common::TempDir;

    let temp_cache = TempDir::new("gog_webcache_full");
    let gog_dir = temp_cache.path().join("GOG.com").join("Galaxy").join("webcache").join("user1").join("gog").join("778899");
    std::fs::create_dir_all(&gog_dir).unwrap();

    let old_appdata = std::env::var("LOCALAPPDATA").ok();
    std::env::set_var("LOCALAPPDATA", temp_cache.path().to_str().unwrap());

    // 1. Vertical cover (>2000 bytes)
    let vert_cover = gog_dir.join("game_glx_vertical_cover.jpg");
    std::fs::write(&vert_cover, vec![0u8; 2500]).unwrap();

    let found_vert = find_local_gog_cover("778899");
    assert_eq!(found_vert, Some(vert_cover.clone()));

    // 2. Fallback cover (_glx_bg_ or _glx_logo)
    std::fs::remove_file(&vert_cover).unwrap();
    let bg_cover = gog_dir.join("game_glx_bg_art.jpg");
    std::fs::write(&bg_cover, vec![0u8; 2500]).unwrap();

    let found_bg = find_local_gog_cover("778899");
    assert_eq!(found_bg, Some(bg_cover));

    // 3. Numeric ID metadata extraction with local cover resolution
    let game_dir = TempDir::new("gog_numeric_game");
    let info_path = game_dir.join("goggame-778899.info");
    std::fs::write(&info_path, r#"{"gameId": 778899, "name": "Numeric GOG Game"}"#).unwrap();

    let (name, poster, gid) = extract_gog_metadata(game_dir.path());
    assert_eq!(name.as_deref(), Some("Numeric GOG Game"));
    assert_eq!(gid.as_deref(), Some("778899"));
    assert!(poster.is_some());

    if let Some(old) = old_appdata {
        std::env::set_var("LOCALAPPDATA", old);
    }
}

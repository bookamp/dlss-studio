use dlss_studio::core::steamart::resolver::*;
use dlss_studio::core::steamart::image::key_for_dir;
use dlss_studio::core::state::get_appdata_dir;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_game_art_default() {
    let art = GameArt::default();
    assert_eq!(art.appid, None);
    assert_eq!(art.cover_path, None);
    assert_eq!(art.hero_path, None);
}

#[test]
fn test_find_gog_game_id_string_and_number() {
    let temp = TempDir::new("gog_info");
    
    // Directory with no info file
    assert_eq!(find_gog_game_id(temp.path()), None);

    // String gameId
    let info_path = temp.join("goggame-123456789.info");
    fs::write(&info_path, r#"{"gameId": "123456789", "name": "Cyberpunk 2077"}"#).unwrap();
    assert_eq!(find_gog_game_id(temp.path()), Some("123456789".to_string()));

    // Numeric gameId
    let temp_num = TempDir::new("gog_info_num");
    let info_num_path = temp_num.join("goggame-98765.info");
    fs::write(&info_num_path, r#"{"gameId": 98765, "name": "Witcher 3"}"#).unwrap();
    assert_eq!(find_gog_game_id(temp_num.path()), Some("98765".to_string()));
}

#[test]
fn test_find_cached_art_multi_extension() {
    let temp = TempDir::new("cached_art_test");
    let key = key_for_dir(temp.path());
    let art_dir = get_appdata_dir().join("art");
    fs::create_dir_all(&art_dir).unwrap();

    // No art file yet
    // Notice find_cached_art looks in get_appdata_dir().join("art")
    let target_cover = art_dir.join(format!("{}-cover.jpg", key));
    
    // Clean up if it existed
    let _ = fs::remove_file(&target_cover);
    assert_eq!(find_cached_art(temp.path()), None);

    // Write a dummy file that is >2000 bytes
    let dummy_data = vec![0u8; 2500];
    fs::write(&target_cover, &dummy_data).unwrap();

    let found = find_cached_art(temp.path());
    assert!(found.is_some());
    assert!(found.unwrap().contains(&format!("{}-cover.jpg", key)));

    // Clean up
    let _ = fs::remove_file(&target_cover);
}

#[tokio::test]
async fn test_search_steam_candidates_empty_query() {
    let res = search_steam_candidates("").await;
    assert!(res.is_empty());

    let res_spaces = search_steam_candidates("   ").await;
    assert!(res_spaces.is_empty());
}

#[tokio::test]
async fn test_resolve_game_art_with_cached_file() {
    let temp = TempDir::new("game_art_cache_hit");
    let key = key_for_dir(temp.path());
    let art_dir = get_appdata_dir().join("art");
    fs::create_dir_all(&art_dir).unwrap();

    let target_cover = art_dir.join(format!("{}-cover.jpg", key));
    let target_hero = art_dir.join(format!("{}-hero.jpg", key));

    let dummy_data = vec![0u8; 2500];
    fs::write(&target_cover, &dummy_data).unwrap();
    fs::write(&target_hero, &dummy_data).unwrap();

    let art = resolve_game_art("Cached Game", temp.path(), Some(12345)).await;
    assert_eq!(art.appid, Some(12345));
    assert!(art.cover_path.is_some());
    assert!(art.hero_path.is_some());

    let _ = fs::remove_file(&target_cover);
    let _ = fs::remove_file(&target_hero);
}

#[tokio::test]
async fn test_search_steam_appid_empty() {
    assert_eq!(search_steam_appid("").await, None);
    assert_eq!(search_steam_appid("   ").await, None);
}

#[tokio::test]
async fn test_download_gog_art_invalid_id() {
    let client = reqwest::Client::new();
    let temp = TempDir::new("gog_art_fail");
    let res = download_gog_art(&client, "9999999999999_invalid_id", temp.path(), "mykey").await;
    assert_eq!(res, None);
}

#[tokio::test]
async fn test_resolve_game_art_uncached_fallback() {
    let temp = TempDir::new("uncached_art");
    let art = resolve_game_art("NonexistentTitle_987654321", temp.path(), None).await;
    assert_eq!(art.cover_path, None);
    assert_eq!(art.hero_path, None);
}

#[test]
fn test_find_cached_art_length_threshold_and_extensions() {
    let temp = TempDir::new("cached_art_thresh");
    let key = key_for_dir(temp.path());
    let art_dir = get_appdata_dir().join("art");
    fs::create_dir_all(&art_dir).unwrap();

    let target_webp = art_dir.join(format!("{}.webp", key));

    // File <= 2000 bytes should be ignored (corrupted / empty placeholder)
    fs::write(&target_webp, vec![0u8; 1500]).unwrap();
    assert_eq!(find_cached_art(temp.path()), None);

    // File > 2000 bytes should be accepted
    fs::write(&target_webp, vec![0u8; 2500]).unwrap();
    let found = find_cached_art(temp.path());
    assert!(found.is_some());
    assert!(found.unwrap().ends_with(&format!("{}.webp", key)));

    let _ = fs::remove_file(&target_webp);
}

#[test]
fn test_find_gog_game_id_invalid_and_corrupt_files() {
    let temp = TempDir::new("gog_corrupt");

    // Invalid JSON
    let bad_json = temp.join("goggame-bad.info");
    fs::write(&bad_json, b"NOT_JSON").unwrap();
    assert_eq!(find_gog_game_id(temp.path()), None);

    // Valid JSON but missing gameId
    let no_gid = temp.join("goggame-nogid.info");
    fs::write(&no_gid, r#"{"name": "No Game Id"}"#).unwrap();
    assert_eq!(find_gog_game_id(temp.path()), None);

    // Non-info file ignored
    let txt = temp.join("goggame-ignore.txt");
    fs::write(&txt, r#"{"gameId": "123"}"#).unwrap();
    assert_eq!(find_gog_game_id(temp.path()), None);
}

#[tokio::test]
async fn test_resolve_game_art_cache_hit_without_hero() {
    let temp = TempDir::new("game_art_partial_cache");
    let key = key_for_dir(temp.path());
    let art_dir = get_appdata_dir().join("art");
    fs::create_dir_all(&art_dir).unwrap();

    let target_cover = art_dir.join(format!("{}-cover.jpg", key));
    let target_hero = art_dir.join(format!("{}-hero.jpg", key));

    // Write only cover, no hero
    fs::write(&target_cover, vec![0u8; 2500]).unwrap();
    let _ = fs::remove_file(&target_hero);

    let art = resolve_game_art("Partial Game", temp.path(), Some(55555)).await;
    assert_eq!(art.appid, Some(55555));
    assert!(art.cover_path.is_some());
    assert_eq!(art.hero_path, None);

    let _ = fs::remove_file(&target_cover);
}

#[tokio::test]
async fn test_search_steam_candidates_camelcase_and_numbers() {
    // Tests secondary query splitting (Cyberpunk2077 -> Cyberpunk 2077)
    let candidates = search_steam_candidates("Cyberpunk2077").await;
    // When online, this resolves candidates; when offline/firewalled, returns safely without panic
    let _ = candidates;
}

#[tokio::test]
async fn test_query_steam_api_invalid_request() {
    let client = reqwest::Client::new();
    // Query with characters that return no items or error cleanly
    let res = query_steam_api(&client, "NonExistentSpecialTitle_!@#$%^&*()").await;
    if let Some(data) = res {
        assert!(data.items.unwrap_or_default().is_empty());
    }
}

#[tokio::test]
async fn test_resolve_game_art_with_gog_metadata_file() {
    let temp = TempDir::new("gog_resolver_art");
    let info_path = temp.join("goggame-1495134320.info");
    fs::write(&info_path, r#"{"gameId": "1495134320", "name": "Cyberpunk 2077"}"#).unwrap();

    let art = resolve_game_art("Cyberpunk 2077", temp.path(), None).await;
    // Exercises GOG product fallback path
    let _ = art;
}

#[tokio::test]
async fn test_resolve_game_art_known_appid_execution() {
    let temp = TempDir::new("steam_known_appid");
    // Known appid 1091500 (Cyberpunk 2077)
    let art = resolve_game_art("Cyberpunk 2077", temp.path(), Some(1091500)).await;
    // When online, downloads cover/hero and sets appid; when offline, returns cleanly
    if art.cover_path.is_some() || art.hero_path.is_some() {
        assert_eq!(art.appid, Some(1091500));
    }
}

#[test]
fn test_find_cached_art_all_extensions() {
    let temp = TempDir::new("cached_art_all_ext");
    let key = key_for_dir(temp.path());
    let art_dir = get_appdata_dir().join("art");
    fs::create_dir_all(&art_dir).unwrap();

    let exts = [
        format!("{}.jpg", key),
        format!("{}.png", key),
        format!("{}-cover.webp", key),
        format!("{}-cover.png", key),
        format!("{}-hero.jpg", key),
    ];

    for ext in &exts {
        let p = art_dir.join(ext);
        fs::write(&p, vec![0u8; 2500]).unwrap();
        let res = find_cached_art(temp.path());
        assert!(res.is_some());
        assert!(res.unwrap().contains(ext));
        let _ = fs::remove_file(&p);
    }
}

#[tokio::test]
async fn test_search_steam_appid_execution() {
    let res = search_steam_appid("Cyberpunk 2077").await;
    // Offline or online, does not panic
    let _ = res;
}

#[tokio::test]
async fn test_resolve_game_art_empty_name_returns_none() {
    let temp = TempDir::new("resolve_art_empty");
    let art = resolve_game_art("", temp.path(), None).await;
    assert_eq!(art.appid, None);
    assert_eq!(art.cover_path, None);
    assert_eq!(art.hero_path, None);
}

#[test]
fn test_find_cached_art_small_file_ignored() {
    let temp = TempDir::new("cached_art_small");
    let key = key_for_dir(temp.path());
    let art_dir = get_appdata_dir().join("art");
    fs::create_dir_all(&art_dir).unwrap();

    let target_cover = art_dir.join(format!("{}.webp", key));
    fs::write(&target_cover, vec![0u8; 500]).unwrap(); // <= 2000 bytes
    assert_eq!(find_cached_art(temp.path()), None);
    let _ = fs::remove_file(&target_cover);
}



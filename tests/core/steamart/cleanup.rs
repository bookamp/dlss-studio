use dlss_studio::core::steamart::cleanup::*;
use dlss_studio::core::steamart::image::key_for_dir;
use dlss_studio::core::scan::GameEntry;
use crate::common::TempDir;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_cleanup_unused_art_engine() {
    let temp = TempDir::new("steamart_clean");
    let temp_art = temp.path();

    let active_dir = PathBuf::from("E:\\Games\\ActiveGame");
    let active_key = key_for_dir(&active_dir);

    let hidden_dir = PathBuf::from("E:\\Games\\HiddenGame");
    let hidden_key = key_for_dir(&hidden_dir);

    // 1. Create active game files
    let active_cover = temp_art.join(format!("{}-cover.jpg", active_key));
    fs::write(&active_cover, b"ACTIVE_GAME_COVER").unwrap();

    // 2. Create active Xbox game file with hashed filename in g.poster (e.g. A Plague Tale)
    let xbox_art = temp_art.join("f376bf8f7f1228c0.png");
    fs::write(&xbox_art, b"XBOX_GAME_ART").unwrap();

    // 3. Create hidden game file
    let hidden_cover = temp_art.join(format!("{}-cover.jpg", hidden_key));
    fs::write(&hidden_cover, b"HIDDEN_GAME_COVER").unwrap();

    // 4. Create protected static files
    let rust_iron = temp_art.join("rust_on_iron.jpg");
    fs::write(&rust_iron, b"STATIC_RUST_IRON").unwrap();
    let vibepollo = temp_art.join("vibepollo_poster_v2.png");
    fs::write(&vibepollo, b"VIBEPOLLO_POSTER").unwrap();

    // 5. Create truly orphaned / stale files
    let orphan1 = temp_art.join("deadbeef12345678-cover.jpg");
    fs::write(&orphan1, b"ORPHAN_COVER").unwrap();
    let orphan2 = temp_art.join("fa1f5c6e987c75c2-hero.jpg");
    fs::write(&orphan2, b"ORPHAN_HERO").unwrap();
    let orphan3 = temp_art.join("cc47a8fff5cbb282.png");
    fs::write(&orphan3, b"ORPHAN_PNG").unwrap();

    // Configure active game entry with xbox_art in g.poster
    let mut active_game = GameEntry::default();
    active_game.dir = active_dir.clone();
    active_game.poster = Some("http://dlss-art.localhost/art/f376bf8f7f1228c0.png".to_string());

    let active_games = vec![active_game];
    let hidden_paths = vec![hidden_dir];
    let custom_posters = std::collections::HashMap::new();

    let stats = cleanup_unused_art_in_dir(&temp_art, &active_games, &hidden_paths, &custom_posters);

    // Verify stats
    assert_eq!(stats.files_removed, 3, "Exactly 3 orphaned files must be purged");
    assert!(stats.bytes_reclaimed > 0);

    // Verify active files were PRESERVED
    assert!(active_cover.exists(), "Active game cover must NOT be deleted");
    assert!(xbox_art.exists(), "Active Xbox game artwork must NOT be deleted");
    assert!(hidden_cover.exists(), "Hidden game artwork must NOT be deleted");
    assert!(rust_iron.exists(), "Protected rust_on_iron.jpg must NOT be deleted");
    assert!(vibepollo.exists(), "Protected vibepollo_poster_v2.png must NOT be deleted");

    // Verify orphaned files were REMOVED
    assert!(!orphan1.exists(), "Orphan 1 must be deleted");
    assert!(!orphan2.exists(), "Orphan 2 must be deleted");
    assert!(!orphan3.exists(), "Orphan 3 must be deleted");
}

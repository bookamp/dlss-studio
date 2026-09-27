use dlss_studio::core::steamart::*;
use dlss_studio::core::scan::GameEntry;
use crate::common::TempDir;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn test_clean_name_tags_and_brackets() {
    assert_eq!(clean_name("Cyberpunk 2077 [v2.12] (DODI Repack)"), "Cyberpunk 2077");
    assert_eq!(clean_name("The.Witcher.3.Wild.Hunt-FitGirl"), "The Witcher 3 Wild Hunt");
    assert_eq!(clean_name("Baldur's Gate 3 (ElAmigos)"), "Baldur's Gate 3");
    assert_eq!(clean_name("Hogwarts Legacy [CODEX]"), "Hogwarts Legacy");
    assert_eq!(clean_name("Starfield [FitGirl Repack]"), "Starfield");
}

#[test]
fn test_norm_title_and_scoring() {
    assert_eq!(norm_title("Cyberpunk 2077: Phantom Liberty"), "cyberpunk 2077 phantom liberty");
    let items = vec![
        StoreItem { id: 1091500, name: Some("Cyberpunk 2077".to_string()), item_type: Some("app".to_string()) },
        StoreItem { id: 2138330, name: Some("Cyberpunk 2077: Phantom Liberty".to_string()), item_type: Some("app".to_string()) },
        StoreItem { id: 9999999, name: Some("Cyberpunk Bonus Pack".to_string()), item_type: None },
    ];
    let best = pick_best(&items, "Cyberpunk 2077");
    assert!(best.is_some());
    assert_eq!(best.unwrap().id, 1091500);

    let best_dlc = pick_best(&items, "Phantom Liberty");
    assert!(best_dlc.is_some());
    assert_eq!(best_dlc.unwrap().id, 2138330);
}

#[test]
fn test_key_for_dir_hashing() {
    let p1 = Path::new("C:\\Games\\Cyberpunk 2077");
    let p2 = Path::new("C:/Games/Cyberpunk 2077");
    assert_eq!(key_for_dir(p1), key_for_dir(p2));
    assert_ne!(key_for_dir(p1), key_for_dir(Path::new("D:\\Games\\Witcher 3")));
}

#[test]
fn test_data_uri_conversion() {
    let bytes = vec![0x89u8; 1024]; // 1 KB buffer > 500 bytes
    let uri = bytes_to_data_uri(&bytes);
    assert!(uri.starts_with("data:image/jpeg;base64,"));
    assert!(uri.len() > 25);

    let temp = TempDir::new("steamart_data_uri");
    let file_p = temp.join("cover.jpg");
    fs::write(&file_p, &bytes).unwrap();
    assert_eq!(file_to_data_uri(&file_p), Some(uri));
    assert_eq!(file_to_data_uri(&temp.join("missing.jpg")), None);
}

#[test]
fn test_file_to_art_uri_and_url_decode() {
    assert_eq!(url_decode("Hello%20World%2BTest"), "Hello World+Test");
    let temp = TempDir::new("steamart_art_uri");
    let file_p = temp.join("test_cover.jpg");
    fs::write(&file_p, b"synthetic-jpeg-data").unwrap();

    let art_uri = file_to_art_uri(&file_p);
    assert!(art_uri.is_some());
    let uri_str = art_uri.unwrap();
    assert!(uri_str.starts_with("http://dlss-art.localhost/art/"));

    // Direct HTTP URI
    let handled_direct = handle_art_request(&uri_str);
    assert!(handled_direct.is_some());
    let (mime, data) = handled_direct.unwrap();
    assert_eq!(mime, "image/jpeg");
    assert_eq!(data, b"synthetic-jpeg-data");

    // Wry internal rewritten URI (dlss-art://localhost/art/...)
    let wry_uri = uri_str.replace("http://dlss-art.localhost/art/", "dlss-art://localhost/art/");
    let handled_wry = handle_art_request(&wry_uri);
    assert!(handled_wry.is_some());
    assert_eq!(handled_wry.unwrap().1, b"synthetic-jpeg-data");

    // Legacy stored URI (dlss-art://art/...)
    let legacy_uri = uri_str.replace("http://dlss-art.localhost/art/", "dlss-art://art/");
    let handled_legacy = handle_art_request(&legacy_uri);
    assert!(handled_legacy.is_some());
    assert_eq!(handled_legacy.unwrap().1, b"synthetic-jpeg-data");

    // normalize_art_uri tests
    assert_eq!(
        normalize_art_uri("dlss-art://art/my-cover.jpg"),
        "http://dlss-art.localhost/art/my-cover.jpg"
    );
    assert_eq!(
        normalize_art_uri("dlss-art://localhost/art/my-cover.jpg"),
        "http://dlss-art.localhost/art/my-cover.jpg"
    );
    assert_eq!(
        normalize_art_uri("http://dlss-art.localhost/art/my-cover.jpg"),
        "http://dlss-art.localhost/art/my-cover.jpg"
    );
}

#[test]
fn test_steam_candidate_ranking_with_editions() {
    let items = vec![
        StoreItem {
            id: 3669870,
            name: Some("CONTROL Resonant".to_string()),
            item_type: Some("app".to_string()),
        },
        StoreItem {
            id: 870780,
            name: Some("CONTROL Ultimate Edition".to_string()),
            item_type: Some("app".to_string()),
        },
    ];

    let best = pick_best(&items, "Control");
    assert!(best.is_some());
    assert_eq!(best.unwrap().id, 870780, "Edition titles like 'CONTROL Ultimate Edition' must be prioritized over unrelated spinoffs");
}

#[test]
fn test_handle_art_request_webp() {
    let temp = TempDir::new("steamart_webp");
    let webp_file = temp.join("cover.webp");
    let _ = fs::write(&webp_file, b"RIFF....WEBPVP8 ");

    let uri_str = file_to_art_uri(&webp_file).expect("Must return art URI");
    assert!(uri_str.ends_with(".webp"));
    let handled = handle_art_request(&uri_str);
    assert!(handled.is_some());
    let (mime, bytes) = handled.unwrap();
    assert_eq!(mime, "image/webp");
    assert_eq!(bytes, b"RIFF....WEBPVP8 ");
}

#[test]
fn test_handle_art_request_static_assets() {
    // Direct http://dlss-art.localhost/assets/...
    let dark = handle_art_request("http://dlss-art.localhost/assets/rust_dark.webp");
    assert!(dark.is_some(), "Must serve rust_dark.webp");
    let (mime_dark, bytes_dark) = dark.unwrap();
    assert_eq!(mime_dark, "image/webp");
    assert!(!bytes_dark.is_empty());

    // Wry rewritten dlss-art://localhost/assets/...
    let light = handle_art_request("dlss-art://localhost/assets/rust_light.webp");
    assert!(light.is_some(), "Must serve rust_light.webp");
    let (mime_light, bytes_light) = light.unwrap();
    assert_eq!(mime_light, "image/webp");
    assert!(!bytes_light.is_empty());
}

#[test]
fn test_extract_art_filename_variations() {
    assert_eq!(
        extract_art_filename("http://dlss-art.localhost/art/f376bf8f7f1228c0.png"),
        Some("f376bf8f7f1228c0.png".to_string())
    );
    assert_eq!(
        extract_art_filename("http://dlss-art.localhost/art/0123456789abcdef-cover.jpg?v=2"),
        Some("0123456789abcdef-cover.jpg".to_string())
    );
    assert_eq!(
        extract_art_filename("dlss-art://art/custom_game_poster.webp#hero"),
        Some("custom_game_poster.webp".to_string())
    );
    assert_eq!(
        extract_art_filename("dlss-art://localhost/art/vibepollo_poster_v2.png"),
        Some("vibepollo_poster_v2.png".to_string())
    );
    assert_eq!(
        extract_art_filename("C:\\Users\\User\\AppData\\Roaming\\dlss-5-studio\\art\\f376bf8f7f1228c0.png"),
        Some("f376bf8f7f1228c0.png".to_string())
    );
    assert_eq!(extract_art_filename("data:image/png;base64,iVBORw0KGgoAAAANSUhEUg"), None);
    assert_eq!(extract_art_filename(""), None);
}

#[test]
fn test_optimize_and_save_cover_image_downscaling() {
    let temp = TempDir::new("steamart_opt");

    // Create an uncompressed 1200x1600 image in memory
    let mut img = image::RgbImage::new(1200, 1600);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgb([180, 70, 30]);
    }
    let src_file = temp.join("raw_large_poster.png");
    img.save_with_format(&src_file, image::ImageFormat::Png).expect("Failed to write test image");

    let dst_file = temp.join("optimized_poster.jpg");
    let res = optimize_and_save_cover_image(&src_file, &dst_file);
    assert!(res.is_ok());
    assert!(dst_file.exists());

    // Verify the output was downscaled to max 600x900
    let decoded = image::open(&dst_file).expect("Must open optimized image");
    assert!(decoded.width() <= 600);
    assert!(decoded.height() <= 900);
}

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

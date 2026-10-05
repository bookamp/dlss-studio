use dlss_studio::core::steamart::image::*;
use crate::common::TempDir;
use std::fs;
use std::path::Path;

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
fn test_optimize_and_save_cover_image_fast_copy_and_formats() {
    let temp = TempDir::new("steamart_fast_copy");

    // 1. Small 200x300 image (already within bounds)
    let img = image::RgbImage::new(200, 300);
    let small_src = temp.join("small.jpg");
    img.save_with_format(&small_src, image::ImageFormat::Jpeg).unwrap();

    let small_dst = temp.join("small_copied.jpg");
    let res = optimize_and_save_cover_image(&small_src, &small_dst);
    assert!(res.is_ok());
    assert!(small_dst.exists());

    // 2. In-place optimization
    let inplace_file = temp.join("inplace.png");
    let mut large_img = image::RgbImage::new(1000, 1000);
    for p in large_img.pixels_mut() {
        *p = image::Rgb([10, 20, 30]);
    }
    large_img.save_with_format(&inplace_file, image::ImageFormat::Png).unwrap();
    let res_inplace = optimize_and_save_cover_image(&inplace_file, &inplace_file);
    assert!(res_inplace.is_ok());

    // 3. WebP and PNG format targets
    let webp_dst = temp.join("target.webp");
    assert!(optimize_and_save_cover_image(&inplace_file, &webp_dst).is_ok());
    assert!(webp_dst.exists());

    // 4. Non-image fallback copy
    let text_src = temp.join("not_an_image.bin");
    fs::write(&text_src, b"NOT_IMAGE").unwrap();
    let text_dst = temp.join("not_an_image_dst.bin");
    assert!(optimize_and_save_cover_image(&text_src, &text_dst).is_ok());
    assert_eq!(fs::read(&text_dst).unwrap(), b"NOT_IMAGE");
}

#[test]
fn test_optimize_and_save_cover_bytes_branches() {
    let temp = TempDir::new("steamart_bytes");

    // 1. Small buffer direct write
    let small_bytes = vec![0x42u8; 100];
    let dst1 = temp.join("small.bin");
    let len1 = optimize_and_save_cover_bytes(&small_bytes, &dst1).unwrap();
    assert_eq!(len1, 100);

    // 2. Large image buffer downscale
    let mut img = image::RgbImage::new(1200, 1200);
    for p in img.pixels_mut() {
        *p = image::Rgb([255, 128, 0]);
    }
    let mut encoded = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut encoded);
    img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();

    let dst2 = temp.join("downscaled_bytes.jpg");
    let res2 = optimize_and_save_cover_bytes(&encoded, &dst2);
    assert!(res2.is_ok());
    assert!(dst2.exists());
}

#[test]
fn test_normalize_art_uri_variations() {
    // 1. data:image pass-through
    assert_eq!(
        normalize_art_uri("data:image/png;base64,ABC"),
        "data:image/png;base64,ABC"
    );

    // 2. http://dlss-art.localhost/ pass-through
    assert_eq!(
        normalize_art_uri("http://dlss-art.localhost/art/cover.jpg"),
        "http://dlss-art.localhost/art/cover.jpg"
    );

    // 3. dlss-art://art/ scheme
    assert_eq!(
        normalize_art_uri("dlss-art://art/mygame.png"),
        "http://dlss-art.localhost/art/mygame.png"
    );

    // 4. dlss-art://localhost/art/ scheme
    assert_eq!(
        normalize_art_uri("dlss-art://localhost/art/game2.webp"),
        "http://dlss-art.localhost/art/game2.webp"
    );

    // 5. http://dlss-art.local/art/ scheme
    assert_eq!(
        normalize_art_uri("http://dlss-art.local/art/game3.jpg"),
        "http://dlss-art.localhost/art/game3.jpg"
    );

    // 6. Unknown raw string fallback
    assert_eq!(
        normalize_art_uri("some_random_string"),
        "some_random_string"
    );
}

#[test]
fn test_file_to_art_uri_lifecycle() {
    let temp = TempDir::new("steamart_file_uri");
    assert_eq!(file_to_art_uri(&temp.join("missing.jpg")), None);

    let img_path = temp.join("real_poster.png");
    let img = image::RgbImage::new(100, 100);
    img.save_with_format(&img_path, image::ImageFormat::Png).unwrap();

    let uri = file_to_art_uri(&img_path);
    assert!(uri.is_some());
    let u = uri.unwrap();
    assert!(u.starts_with("http://dlss-art.localhost/art/"));
}

#[test]
fn test_optimize_and_save_cover_bytes_png_and_webp() {
    let temp = TempDir::new("steamart_bytes_formats");
    let img = image::RgbImage::new(800, 1200);
    let mut raw_png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut raw_png), image::ImageFormat::Png).unwrap();

    // Pad buffer to exceed 300,000 bytes threshold
    if raw_png.len() < 300_000 {
        raw_png.resize(300_001, 0xFF);
    }

    // 1. Target PNG
    let dst_png = temp.join("output.png");
    let res_png = optimize_and_save_cover_bytes(&raw_png, &dst_png);
    assert!(res_png.is_ok());

    // 2. Target WebP
    let dst_webp = temp.join("output.webp");
    let res_webp = optimize_and_save_cover_bytes(&raw_png, &dst_webp);
    assert!(res_webp.is_ok());

    // 3. Invalid image bytes fallback to raw write
    let bad_bytes = vec![0x12; 300_050];
    let dst_bad = temp.join("corrupt.jpg");
    let res_bad = optimize_and_save_cover_bytes(&bad_bytes, &dst_bad);
    assert!(res_bad.is_ok());
    assert_eq!(dst_bad.metadata().unwrap().len(), 300_050);
}

#[test]
fn test_file_to_art_uri_inside_cache() {
    let art_dir = dlss_studio::core::state::get_appdata_dir().join("art");
    let _ = std::fs::create_dir_all(&art_dir);
    let in_cache = art_dir.join("test_in_cache.png");
    let _ = std::fs::write(&in_cache, b"fake_png_data");

    let uri = file_to_art_uri(&in_cache);
    assert!(uri.is_some());
    assert_eq!(uri.unwrap(), "http://dlss-art.localhost/art/test_in_cache.png");

    let _ = std::fs::remove_file(&in_cache);
}

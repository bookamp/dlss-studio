use dlss_studio::core::steamart::handler::*;
use dlss_studio::core::steamart::image::*;
use dlss_studio::core::steamart::query::*;
use crate::common::TempDir;
use std::fs;

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
fn test_mime_from_extension_all_types() {
    assert_eq!(mime_from_extension("poster.webp"), "image/webp");
    assert_eq!(mime_from_extension("poster.png"), "image/png");
    assert_eq!(mime_from_extension("icon.svg"), "image/svg+xml");
    assert_eq!(mime_from_extension("cover.jpg"), "image/jpeg");
    assert_eq!(mime_from_extension("cover.jpeg"), "image/jpeg");
    assert_eq!(mime_from_extension("unknown.xyz"), "image/jpeg");
}

#[test]
fn test_handle_art_request_file_param_and_missing() {
    let temp = TempDir::new("steamart_file_param");
    let target = temp.join("direct_load.png");
    fs::write(&target, b"PNG_RAW").unwrap();

    let encoded_path = url_encode(target.to_str().unwrap());
    let req_url = format!("http://dlss-art.localhost/?file={}", encoded_path);
    let handled = handle_art_request(&req_url);
    assert!(handled.is_some());
    let (mime, bytes) = handled.unwrap();
    assert_eq!(mime, "image/png");
    assert_eq!(bytes, b"PNG_RAW");

    // Missing file returns None
    assert_eq!(handle_art_request("http://dlss-art.localhost/art/does_not_exist.jpg"), None);
    assert_eq!(handle_art_request("http://dlss-art.localhost/assets/unknown_asset.png"), None);
}

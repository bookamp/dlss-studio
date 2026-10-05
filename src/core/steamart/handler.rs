//! Protocol request handler for dlss-art:// and static asset streaming.

use std::fs;
use crate::core::state::get_appdata_dir;
use super::query::url_decode;

static EMBEDDED_RUST_DARK: &[u8] = include_bytes!("../../../assets/rust_dark.webp");
static EMBEDDED_RUST_LIGHT: &[u8] = include_bytes!("../../../assets/rust_light.webp");

pub fn mime_from_extension(filename: &str) -> &'static str {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if ext.eq_ignore_ascii_case("webp") {
        "image/webp"
    } else if ext.eq_ignore_ascii_case("png") {
        "image/png"
    } else if ext.eq_ignore_ascii_case("svg") {
        "image/svg+xml"
    } else {
        "image/jpeg"
    }
}

/// Serves static application assets located in the `assets/` folder, falling back to compile-time
/// embedded binaries so standalone portable single-file executables remain 100% self-contained.
pub fn serve_static_asset(name: &str) -> Option<(String, Vec<u8>)> {
    let clean_name = name.split('?').next().unwrap_or(name).trim_start_matches('/');

    // 1. Try reading directly from disk if assets/ exists alongside cwd or exe
    let mut candidate_paths = vec![std::path::PathBuf::from("assets").join(clean_name)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidate_paths.push(parent.join("assets").join(clean_name));
        }
    }

    for path in &candidate_paths {
        if path.is_file() {
            if let Ok(bytes) = fs::read(path) {
                let mime = mime_from_extension(clean_name);
                return Some((mime.to_string(), bytes));
            }
        }
    }

    // 2. Embedded compile-time fallback for standalone portable executable distribution
    match clean_name {
        "rust_dark.webp" => Some(("image/webp".to_string(), EMBEDDED_RUST_DARK.to_vec())),
        "rust_light.webp" => Some(("image/webp".to_string(), EMBEDDED_RUST_LIGHT.to_vec())),
        _ => None,
    }
}

/// Handles requests to the custom dlss-art protocol, reading cached posters directly from disk.
/// Accepts URLs from Wry's internal rewrite (dlss-art://localhost/art/...), direct HTTP (http://dlss-art.localhost/art/...),
/// or legacy (dlss-art://art/...), as well as static application assets (http://dlss-art.localhost/assets/...).
pub fn handle_art_request(uri: &str) -> Option<(String, Vec<u8>)> {
    let clean = uri.split('?').next().unwrap_or(uri);

    // 1. Static application assets (/assets/...)
    if let Some(idx) = clean.find("/assets/") {
        let asset_name = &clean[idx + 8..];
        return serve_static_asset(asset_name);
    }
    let without_proto = clean
        .trim_start_matches("dlss-art://")
        .trim_start_matches("dlss-art:")
        .trim_start_matches("http://dlss-art.localhost/")
        .trim_start_matches("http://dlss-art.local/");
    if let Some(stripped) = without_proto.strip_prefix("assets/") {
        return serve_static_asset(stripped);
    }

    // 2. Game art poster cache
    let art_dir = get_appdata_dir().join("art");

    let target_file = if let Some(idx) = uri.find("file=") {
        let raw = &uri[idx + 5..];
        let raw_file = raw.split('&').next().unwrap_or(raw);
        let decoded = url_decode(raw_file);
        std::path::PathBuf::from(decoded)
    } else {
        let filename = if let Some(idx) = clean.find("/art/") {
            &clean[idx + 5..]
        } else if let Some(stripped) = clean.strip_prefix("art/") {
            stripped
        } else {
            without_proto.trim_start_matches("localhost/").trim_start_matches('/')
        };
        art_dir.join(filename.trim_start_matches('/'))
    };

    if target_file.is_file() {
        if let Ok(bytes) = fs::read(&target_file) {
            let mime = if target_file.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("png")).unwrap_or(false) {
                "image/png"
            } else if target_file.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("webp")).unwrap_or(false) {
                "image/webp"
            } else {
                "image/jpeg"
            };
            return Some((mime.to_string(), bytes));
        }
    }
    None
}

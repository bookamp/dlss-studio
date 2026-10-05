//! Cover image optimization, format conversion, and art URI generation.

use std::fs;
use std::path::Path;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use crate::core::state::get_appdata_dir;

/// Generates a filesystem-safe cache key for a game directory.
pub fn key_for_dir(dir: &Path) -> String {
    let s = dir.to_string_lossy().to_lowercase().replace('/', "\\");
    let mut hasher = sha2::Sha256::default();
    use sha2::Digest;
    hasher.update(s.as_bytes());
    hex::encode(&hasher.finalize()[..8])
}

/// Helper to read file and convert to base64 data URI (legacy fallback)
pub fn file_to_data_uri(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 500 {
        return None;
    }
    let encoded = BASE64_STANDARD.encode(&bytes);
    Some(format!("data:image/jpeg;base64,{}", encoded))
}

pub fn bytes_to_data_uri(bytes: &[u8]) -> String {
    let encoded = BASE64_STANDARD.encode(bytes);
    format!("data:image/jpeg;base64,{}", encoded)
}

/// Saves and optimizes an image file for use as game artwork.
/// Oversized images (> 400 KB or dimensions > 600x900) are downscaled to fit within 600x900.
pub fn optimize_and_save_cover_image(src_path: &Path, dst_path: &Path) -> std::io::Result<u64> {
    let src_ext = src_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
    let dst_ext = dst_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());

    // If source and destination format match, file is small (< 300 KB), and dimensions fit, fast copy
    if src_ext == dst_ext && src_path != dst_path {
        if let Ok(meta) = src_path.metadata() {
            if meta.len() < 300_000 {
                if let Ok((w, h)) = image::image_dimensions(src_path) {
                    if w <= 600 && h <= 900 {
                        return fs::copy(src_path, dst_path);
                    }
                }
            }
        }
    }

    if let Ok(img) = image::open(src_path) {
        let (w, h) = (img.width(), img.height());
        let resized = if w > 600 || h > 900 {
            img.resize(600, 900, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        let ext = dst_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
        let fmt = match ext.as_deref() {
            Some("webp") => image::ImageFormat::WebP,
            Some("png") => image::ImageFormat::Png,
            _ => image::ImageFormat::Jpeg,
        };

        let write_target = if src_path == dst_path {
            dst_path.with_extension("tmp_opt")
        } else {
            dst_path.to_path_buf()
        };

        if resized.save_with_format(&write_target, fmt).is_ok() {
            if write_target != dst_path {
                let _ = fs::rename(&write_target, dst_path);
            }
            if let Ok(m) = dst_path.metadata() {
                return Ok(m.len());
            }
        }
        if write_target != dst_path && write_target.exists() {
            let _ = fs::remove_file(&write_target);
        }
    }

    if src_path != dst_path {
        fs::copy(src_path, dst_path)
    } else {
        Ok(src_path.metadata().map(|m| m.len()).unwrap_or(0))
    }
}

/// Saves and optimizes raw image bytes (e.g. from user file picker) to dst_path.
pub fn optimize_and_save_cover_bytes(bytes: &[u8], dst_path: &Path) -> std::io::Result<u64> {
    if bytes.len() < 300_000 {
        fs::write(dst_path, bytes)?;
        return Ok(bytes.len() as u64);
    }

    if let Ok(img) = image::load_from_memory(bytes) {
        let (w, h) = (img.width(), img.height());
        let resized = if w > 600 || h > 900 {
            img.resize(600, 900, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        let ext = dst_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
        let fmt = match ext.as_deref() {
            Some("webp") => image::ImageFormat::WebP,
            Some("png") => image::ImageFormat::Png,
            _ => image::ImageFormat::Jpeg,
        };

        if resized.save_with_format(dst_path, fmt).is_ok() {
            if let Ok(m) = dst_path.metadata() {
                return Ok(m.len());
            }
        }
    }

    fs::write(dst_path, bytes)?;
    Ok(bytes.len() as u64)
}

/// Helper to convert a local file to a lightweight HTTP URI served by Wry's custom desktop protocol
pub fn file_to_art_uri(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let art_dir = get_appdata_dir().join("art");
    let _ = fs::create_dir_all(&art_dir);

    // If already in art cache, reference filename directly
    if let Ok(rel) = path.strip_prefix(&art_dir) {
        let s = rel.to_string_lossy().replace('\\', "/");
        return Some(format!("http://dlss-art.localhost/art/{}", s));
    }

    // Otherwise, copy/optimize into art cache under safe key
    let key = key_for_dir(path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let cached = art_dir.join(format!("{}.{}", key, ext));
    if !cached.exists() {
        let _ = optimize_and_save_cover_image(path, &cached);
    } else if let Ok(meta) = cached.metadata() {
        // If the cached file is an oversized legacy uncompressed image (> 1 MB), optimize it in place
        if meta.len() > 1_000_000 {
            let _ = optimize_and_save_cover_image(&cached, &cached);
        }
    }
    Some(format!("http://dlss-art.localhost/art/{}.{}", key, ext))
}

/// Extracts the underlying cached art filename from an art URI or path.
/// E.g. "http://dlss-art.localhost/art/f376bf8f7f1228c0.png" -> Some("f376bf8f7f1228c0.png")
/// "dlss-art://art/0123456789abcdef-cover.jpg" -> Some("0123456789abcdef-cover.jpg")
pub fn extract_art_filename(uri_or_path: &str) -> Option<String> {
    if uri_or_path.is_empty() || uri_or_path.starts_with("data:image/") {
        return None;
    }
    let s = uri_or_path.split('?').next().unwrap_or(uri_or_path);
    let s = s.split('#').next().unwrap_or(s);
    let trimmed = s.trim_end_matches(['/', '\\']);

    if let Some(pos) = trimmed.rfind("/art/") {
        let name = &trimmed[pos + 5..];
        let clean = name.trim_matches(['/', '\\']);
        if !clean.is_empty() && !clean.contains('/') && !clean.contains('\\') {
            return Some(clean.to_string());
        }
    }
    if let Some(pos) = trimmed.rfind("dlss-art://") {
        let name = &trimmed[pos + 11..];
        let clean = name.trim_start_matches("localhost/").trim_start_matches("art/").trim_matches(['/', '\\']);
        if !clean.is_empty() && !clean.contains('/') && !clean.contains('\\') {
            return Some(clean.to_string());
        }
    }

    let p = Path::new(trimmed);
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Normalizes any poster URI (legacy dlss-art://, raw local file paths, or current scheme)
/// into a Wry-compatible http://dlss-art.localhost/art/... URL.
pub fn normalize_art_uri(uri: &str) -> String {
    if uri.starts_with("data:image/") {
        return uri.to_string();
    }
    if uri.starts_with("http://dlss-art.localhost/") {
        return uri.to_string();
    }
    if let Some(rest) = uri.strip_prefix("dlss-art://art/") {
        return format!("http://dlss-art.localhost/art/{}", rest.trim_start_matches('/'));
    }
    if let Some(rest) = uri.strip_prefix("dlss-art://") {
        let clean = rest
            .trim_start_matches("localhost/")
            .trim_start_matches("local/")
            .trim_start_matches("art/");
        return format!("http://dlss-art.localhost/art/{}", clean.trim_start_matches('/'));
    }
    if let Some(rest) = uri.strip_prefix("http://dlss-art.local/art/") {
        return format!("http://dlss-art.localhost/art/{}", rest.trim_start_matches('/'));
    }
    // If it's a raw filesystem path, convert it via file_to_art_uri
    let p = Path::new(uri);
    if p.is_file() {
        if let Some(art_url) = file_to_art_uri(p) {
            return art_url;
        }
    }
    uri.to_string()
}

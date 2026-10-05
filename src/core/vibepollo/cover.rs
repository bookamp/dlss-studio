//! Cover art extraction and caching for Vibepollo Big Picture posters.

use std::fs;
use std::path::PathBuf;

/// Prepares and exports the high-resolution 600x900 vertical poster image to the local AppData folder.
pub fn ensure_vibepollo_cover_art() -> Option<PathBuf> {
    let art_dir = crate::core::state::get_appdata_dir().join("art");
    let _ = fs::create_dir_all(&art_dir);
    let poster_path = art_dir.join("vibepollo_poster_v2.png");
    let legacy_cover = art_dir.join("vibepollo_cover.png");
    if legacy_cover.exists() {
        let _ = fs::remove_file(legacy_cover);
    }

    let poster_bytes = include_bytes!("../../../assets/vibepollo_poster.webp");
    let needs_update = if !poster_path.exists() {
        true
    } else {
        match fs::read(&poster_path) {
            Ok(existing_bytes) => {
                // Check for valid PNG magic bytes and reasonable size (> 50 KB)
                existing_bytes.len() < 50_000 || &existing_bytes[..8.min(existing_bytes.len())] != b"\x89PNG\r\n\x1a\n"
            }
            Err(_) => true,
        }
    };

    if needs_update {
        if let Ok(img) = image::load_from_memory_with_format(poster_bytes, image::ImageFormat::WebP) {
            let _ = img.save_with_format(&poster_path, image::ImageFormat::Png);
        } else {
            let _ = fs::write(&poster_path, poster_bytes);
        }
    }

    if poster_path.exists() {
        Some(poster_path)
    } else {
        None
    }
}

//! Art cache cleanup engine for discovering and removing orphaned poster files.

use std::fs;
use std::path::Path;
use crate::core::state::get_appdata_dir;
use super::image::{extract_art_filename, key_for_dir};

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArtCleanupStats {
    pub files_scanned: usize,
    pub files_removed: usize,
    pub bytes_reclaimed: u64,
}

/// Identifies and removes all artwork files in the art cache that do not belong
/// to any active or hidden game in the user's library, or static whitelisted assets.
pub fn cleanup_unused_art_in_dir<P: AsRef<Path>>(
    art_dir: &Path,
    active_games: &[crate::core::scan::GameEntry],
    hidden_paths: &[P],
    custom_posters: &std::collections::HashMap<String, String>,
) -> ArtCleanupStats {
    if !art_dir.is_dir() {
        return ArtCleanupStats::default();
    }

    // 1. Build set of valid directory hash keys
    let mut valid_keys = std::collections::HashSet::new();
    for g in active_games {
        valid_keys.insert(key_for_dir(&g.dir));
    }
    for p in hidden_paths {
        valid_keys.insert(key_for_dir(p.as_ref()));
    }

    // 2. Build set of explicit filenames referenced in posters or state
    let mut explicit_filenames = std::collections::HashSet::new();
    for g in active_games {
        if let Some(ref p) = g.poster {
            if let Some(name) = extract_art_filename(p) {
                explicit_filenames.insert(name.to_lowercase());
            }
        }
    }
    for (_, poster_val) in custom_posters {
        if let Some(name) = extract_art_filename(poster_val) {
            explicit_filenames.insert(name.to_lowercase());
        }
    }

    // 3. Protected static whitelist
    let protected_static = [
        "rust_on_iron.jpg",
        "vibepollo_poster_v2.png",
        "vibepollo_poster_v2.webp",
        "rust_dark.webp",
        "rust_light.webp",
    ];

    // 4. Iterate art/ and delete unreferenced files
    let mut stats = ArtCleanupStats::default();
    if let Ok(entries) = fs::read_dir(art_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            stats.files_scanned += 1;
            let filename = entry.file_name().to_string_lossy().to_string();
            let filename_lower = filename.to_lowercase();

            // Check if protected static asset
            if protected_static.iter().any(|&s| s.eq_ignore_ascii_case(&filename_lower)) {
                continue;
            }

            // Check if explicitly referenced filename
            if explicit_filenames.contains(&filename_lower) {
                continue;
            }

            // Check if filename starts with or matches any active game key
            let is_active_game_art = valid_keys.iter().any(|k| filename_lower.starts_with(k));
            if is_active_game_art {
                continue;
            }

            // Orphaned file!
            let file_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if fs::remove_file(&path).is_ok() {
                stats.files_removed += 1;
                stats.bytes_reclaimed += file_size;
                crate::core::logger::info(
                    "art",
                    &format!("Purged orphaned artwork: {} (reclaimed {} KB)", filename, file_size / 1024),
                );
            }
        }
    }

    stats
}

/// Helper that runs cleanup against the application's configured art directory.
pub fn cleanup_unused_art<P: AsRef<Path>>(
    active_games: &[crate::core::scan::GameEntry],
    hidden_paths: &[P],
    custom_posters: &std::collections::HashMap<String, String>,
) -> ArtCleanupStats {
    let art_dir = get_appdata_dir().join("art");
    cleanup_unused_art_in_dir(&art_dir, active_games, hidden_paths, custom_posters)
}

/// Computes the total byte size of an art directory.
pub fn get_art_cache_size_in_dir(art_dir: &Path) -> u64 {
    if !art_dir.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(art_dir) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    total += m.len();
                }
            }
        }
    }
    total
}

/// Computes the total byte size of the application's configured art directory.
pub fn get_art_cache_size() -> u64 {
    get_art_cache_size_in_dir(&get_appdata_dir().join("art"))
}

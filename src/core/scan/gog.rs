#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY};

use super::{GameEntry, scan_game_directory};
#[cfg(windows)]
use super::{win32_enum_subkeys, win32_read_reg_string};

pub fn find_local_gog_cover(game_id: &str) -> Option<PathBuf> {
    let mut base_dirs = Vec::new();
    if let Ok(prog_data) = std::env::var("ProgramData") {
        base_dirs.push(PathBuf::from(prog_data).join("GOG.com").join("Galaxy").join("webcache"));
    }
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        base_dirs.push(PathBuf::from(local_app).join("GOG.com").join("Galaxy").join("webcache"));
    }

    for base in base_dirs {
        if !base.is_dir() {
            continue;
        }
        if let Ok(users) = fs::read_dir(&base) {
            for user in users.flatten() {
                let gog_dir = user.path().join("gog").join(game_id);
                if gog_dir.is_dir() {
                    if let Ok(files) = fs::read_dir(&gog_dir) {
                        let mut fallbacks = Vec::new();
                        for f in files.flatten() {
                            let p = f.path();
                            let fname = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                            if fname.contains("_glx_vertical_cover") {
                                if let Ok(meta) = p.metadata() {
                                    if meta.len() > 2000 {
                                        return Some(p);
                                    }
                                }
                            } else if fname.contains("_glx_bg_") || fname.contains("_glx_logo") {
                                if let Ok(meta) = p.metadata() {
                                    if meta.len() > 2000 {
                                        fallbacks.push(p);
                                    }
                                }
                            }
                        }
                        if let Some(fb) = fallbacks.into_iter().next() {
                            return Some(fb);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn extract_gog_metadata(dir: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let mut resolved_name = None;
    let mut resolved_poster = None;
    let mut resolved_game_id = None;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_lowercase();
            if fname.starts_with("goggame-") && fname.ends_with(".info") {
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if resolved_name.is_none() {
                            if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                                if !n.trim().is_empty() {
                                    resolved_name = Some(n.trim().to_string());
                                }
                            }
                        }
                        if resolved_game_id.is_none() {
                            if let Some(gid) = val.get("gameId").and_then(|v| v.as_str()) {
                                resolved_game_id = Some(gid.trim().to_string());
                            } else if let Some(gid_num) = val.get("gameId").and_then(|v| v.as_i64()) {
                                resolved_game_id = Some(gid_num.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    if resolved_game_id.is_none() || resolved_name.is_none() {
        let subkeys = win32_enum_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\GOG.com\Games", KEY_READ | KEY_WOW64_32KEY);
        let norm_dir = crate::core::state::normalize_game_path(dir);
        for game_id in subkeys {
            let subkey_path = format!(r"SOFTWARE\GOG.com\Games\{}", game_id);
            if let Some(path_str) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "path", KEY_READ | KEY_WOW64_32KEY) {
                if crate::core::state::normalize_path_str(&path_str) == norm_dir {
                    if resolved_game_id.is_none() {
                        resolved_game_id = Some(game_id.clone());
                    }
                    if resolved_name.is_none() {
                        if let Some(gname) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "gameName", KEY_READ | KEY_WOW64_32KEY) {
                            if !gname.trim().is_empty() {
                                resolved_name = Some(gname.trim().to_string());
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    if let Some(ref gid) = resolved_game_id {
        if let Some(cover_path) = find_local_gog_cover(gid) {
            resolved_poster = crate::core::steamart::file_to_art_uri(&cover_path);
        }
    }

    (resolved_name, resolved_poster, resolved_game_id)
}

pub fn discover_gog() -> Vec<GameEntry> {
    let mut games = Vec::new();
    #[cfg(windows)]
    {
        let subkeys = win32_enum_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\GOG.com\Games", KEY_READ | KEY_WOW64_32KEY);
        let mut seen_paths = std::collections::HashSet::new();
        for game_id in subkeys {
            let subkey_path = format!(r"SOFTWARE\GOG.com\Games\{}", game_id);
            if let Some(path_str) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "path", KEY_READ | KEY_WOW64_32KEY) {
                let norm = crate::core::state::normalize_path_str(&path_str);
                if !seen_paths.insert(norm) {
                    continue;
                }
                let gdir = PathBuf::from(path_str.trim());
                if gdir.exists() {
                    if let Some(mut game) = scan_game_directory(&gdir) {
                        game.launcher = "GOG".to_string();
                        if let Some(gname) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "gameName", KEY_READ | KEY_WOW64_32KEY) {
                            if !gname.trim().is_empty() {
                                game.name = gname.trim().to_string();
                            }
                        }
                        games.push(game);
                    }
                }
            }
        }
    }
    games
}

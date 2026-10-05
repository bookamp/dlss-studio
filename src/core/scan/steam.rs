#![allow(dead_code)]

use std::fs;
use std::path::Path;
use regex::Regex;

#[cfg(windows)]
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, KEY_READ};

use super::{GameEntry, scan_game_directory};
use super::heuristics::is_not_a_game_title;
#[cfg(windows)]
use super::win32_read_reg_string;

pub fn discover_steam() -> Vec<GameEntry> {
    #[cfg(windows)]
    let steam_path = win32_read_reg_string(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath", KEY_READ);
    #[cfg(not(windows))]
    let steam_path: Option<String> = None;

    let Some(steam_root) = steam_path else {
        return Vec::new();
    };

    discover_steam_from_path(Path::new(&steam_root))
}

pub fn discover_steam_from_path(steam_root: &Path) -> Vec<GameEntry> {
    let mut games = Vec::new();
    let mut libraries = Vec::new();
    let mut seen_libs = std::collections::HashSet::new();

    let vdf_path = steam_root.join("steamapps").join("libraryfolders.vdf");

    if let Ok(vdf_content) = fs::read_to_string(&vdf_path) {
        let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
        for cap in re.captures_iter(&vdf_content) {
            let lib = cap[1].replace(r"\\", r"\");
            let clean_lib = crate::core::state::clean_path_separators(Path::new(&lib));
            let norm_lib = crate::core::state::normalize_game_path(&clean_lib);
            if seen_libs.insert(norm_lib) {
                libraries.push(clean_lib);
            }
        }
    }

    // Fallback if libraryfolders.vdf was missing or did not include steam_root
    let clean_root = crate::core::state::clean_path_separators(steam_root);
    let norm_root = crate::core::state::normalize_game_path(&clean_root);
    if seen_libs.insert(norm_root) {
        libraries.push(clean_root);
    }

    let mut seen_dirs = std::collections::HashSet::new();
    for lib in libraries {
        let apps_dir = lib.join("steamapps");
        let Ok(entries) = fs::read_dir(&apps_dir) else { continue; };
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with("appmanifest_") && fname.ends_with(".acf") {
                if let Ok(acf_text) = fs::read_to_string(entry.path()) {
                    let appid = Regex::new(r#""appid"\s+"([^"]+)""#).ok().and_then(|r| r.captures(&acf_text).map(|c| c[1].to_string()));
                    let installdir = Regex::new(r#""installdir"\s+"([^"]+)""#).ok().and_then(|r| r.captures(&acf_text).map(|c| c[1].to_string()));
                    let gname = Regex::new(r#""name"\s+"([^"]+)""#).ok().and_then(|r| r.captures(&acf_text).map(|c| c[1].to_string()));

                    if let (Some(aid), Some(idir)) = (appid, installdir) {
                        if let Some(ref title) = gname {
                            if is_not_a_game_title(title) {
                                continue;
                            }
                        }
                        let game_dir = apps_dir.join("common").join(&idir);
                        let norm = crate::core::state::normalize_game_path(&game_dir);
                        if !seen_dirs.insert(norm) {
                            continue;
                        }
                        if game_dir.exists() {
                            if let Some(mut game) = scan_game_directory(&game_dir) {
                                game.launcher = "Steam".to_string();
                                if let Some(real_name) = gname {
                                    game.name = real_name;
                                }
                                let poster_path = steam_root.join("appcache").join("librarycache").join(&aid).join("library_600x900.jpg");
                                if poster_path.exists() {
                                    game.poster = crate::core::steamart::file_to_art_uri(&poster_path);
                                }
                                games.push(game);
                            }
                        }
                    }
                }
            }
        }
    }
    games
}

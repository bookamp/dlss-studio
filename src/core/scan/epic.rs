#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use super::{GameEntry, scan_game_directory};

pub fn discover_epic() -> Vec<GameEntry> {
    discover_epic_from_manifests(Path::new(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests"))
}

pub fn discover_epic_from_manifests(manifests: &Path) -> Vec<GameEntry> {
    let mut games = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir(manifests) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "item").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(loc) = val.get("InstallLocation").and_then(|v| v.as_str()) {
                            let norm = crate::core::state::normalize_path_str(loc);
                            if !seen_paths.insert(norm) {
                                continue;
                            }
                            let gdir = PathBuf::from(loc);
                            if gdir.exists() {
                                if let Some(mut game) = scan_game_directory(&gdir) {
                                    game.launcher = "Epic Games".to_string();
                                    if let Some(dname) = val.get("DisplayName").and_then(|v| v.as_str()) {
                                        game.name = dname.to_string();
                                    }
                                    games.push(game);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    games
}


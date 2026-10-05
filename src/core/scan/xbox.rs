#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
#[cfg(windows)]
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, KEY_READ};

use super::{GameEntry, scan_game_directory};
use super::heuristics::is_installer_or_helper;
use crate::core::pe::inspect_pe;
#[cfg(windows)]
use super::{win32_enum_subkeys, win32_read_reg_string, get_fixed_drives};
#[cfg(not(windows))]
use super::get_fixed_drives;

#[derive(Debug, Clone)]
pub struct XboxExe {
    pub path: PathBuf,
    pub rel: String,
    pub name: String,
    pub bitness: u32,
}

pub fn xbox_executables(game_dir: &Path) -> Vec<XboxExe> {
    let mut configs = Vec::new();
    let root_cfg = game_dir.join("MicrosoftGame.config");
    if root_cfg.exists() {
        configs.push(root_cfg);
    }
    let content_cfg = game_dir.join("Content").join("MicrosoftGame.config");
    if content_cfg.exists() {
        configs.push(content_cfg);
    }

    let mut found = Vec::new();
    let exe_tag_re = Regex::new(r#"(?i)<Executable\b([^>]*)/?>"#).unwrap();
    let name_attr_re = Regex::new(r#"(?i)\bName\s*=\s*"([^"]+)""#).unwrap();
    let arch_attr_re = Regex::new(r#"(?i)\bArchitecture\s*=\s*"([^"]+)""#).unwrap();
    let proc_arch_re = Regex::new(r#"(?i)<ProcessorArchitecture>\s*([^<]+)\s*</ProcessorArchitecture>"#).unwrap();

    for config in configs {
        let Ok(text) = fs::read_to_string(&config) else { continue; };
        let cfg_dir = config.parent().unwrap_or(game_dir);

        let mut default_bitness = 64;
        if let Some(caps) = proc_arch_re.captures(&text) {
            let arch = caps.get(1).map(|m| m.as_str().trim().to_lowercase()).unwrap_or_default();
            if arch == "x86" {
                default_bitness = 32;
            }
        }

        for cap in exe_tag_re.captures_iter(&text) {
            let tag_str = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let name = if let Some(nc) = name_attr_re.captures(tag_str) {
                nc.get(1).map(|m| m.as_str().trim()).unwrap_or("")
            } else {
                continue;
            };

            let base_name = Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or(name);
            if base_name.eq_ignore_ascii_case("gamelaunchhelper.exe") || base_name.to_lowercase().starts_with("gamelaunchhelper") {
                continue;
            }

            let bitness = if let Some(ac) = arch_attr_re.captures(tag_str) {
                let arch = ac.get(1).map(|m| m.as_str().trim().to_lowercase()).unwrap_or_default();
                if arch == "x86" { 32 } else { 64 }
            } else {
                default_bitness
            };

            let full = cfg_dir.join(name.replace('/', "\\"));
            let rel = full.strip_prefix(game_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| base_name.to_string());

            found.push(XboxExe {
                path: full,
                rel,
                name: base_name.to_string(),
                bitness,
            });
        }
    }
    found
}

/// Discovers officially declared game executables from Xbox configs (MicrosoftGame.config, appxmanifest.xml)
/// and GOG manifests (goggame-*.info).
pub fn declared_executables(dir: &Path) -> (Vec<XboxExe>, Option<String>) {
    let mut found = xbox_executables(dir);
    let mut launcher = if !found.is_empty() {
        Some("Xbox".to_string())
    } else {
        None
    };

    // GOG manifests: goggame-*.info
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("goggame-") && name.ends_with(".info") {
                if launcher.is_none() {
                    launcher = Some("GOG".to_string());
                }
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(tasks) = val.get("playTasks").and_then(|v| v.as_array()) {
                            for task in tasks {
                                if let Some(cat) = task.get("category").and_then(|c| c.as_str()) {
                                    if cat != "game" {
                                        continue;
                                    }
                                }
                                if let Some(path_str) = task.get("path").and_then(|p| p.as_str()) {
                                    let full = dir.join(path_str.replace('/', "\\"));
                                    if full.is_file() {
                                        let file_name = full.file_name().unwrap_or_default().to_string_lossy().to_string();
                                        if !is_installer_or_helper(&file_name) && !found.iter().any(|x| x.path == full) {
                                            let pe_opt = inspect_pe(&full);
                                            let bitness = pe_opt.as_ref().map(|p| p.bitness).unwrap_or(64);
                                            let rel = full.strip_prefix(dir)
                                                .map(|p| p.to_string_lossy().to_string())
                                                .unwrap_or_else(|_| path_str.to_string());
                                            found.push(XboxExe {
                                                path: full,
                                                rel,
                                                name: file_name,
                                                bitness,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (found, launcher)
}

pub fn extract_xbox_metadata(dir: &Path) -> (Option<String>, Option<String>) {
    let display_name_re = Regex::new(r#"(?i)DefaultDisplayName\s*=\s*"([^"]+)""#).unwrap();
    let display_name_tag_re = Regex::new(r#"(?i)<DisplayName>\s*([^<]+)\s*</DisplayName>"#).unwrap();
    let splash_re = Regex::new(r#"(?i)SplashScreenImage\s*=\s*"([^"]+)""#).unwrap();
    let logo_re = Regex::new(r#"(?i)Square150x150Logo\s*=\s*"([^"]+)""#).unwrap();
    let store_logo_re = Regex::new(r#"(?i)StoreLogo\s*=\s*"([^"]+)""#).unwrap();

    let mut resolved_name = None;
    let mut resolved_poster = None;

    let configs = [
        dir.join("MicrosoftGame.config"),
        dir.join("Content").join("MicrosoftGame.config"),
        dir.join("appxmanifest.xml"),
        dir.join("AppxManifest.xml"),
    ];

    for cfg in &configs {
        if let Ok(cfg_text) = fs::read_to_string(cfg) {
            if resolved_name.is_none() {
                if let Some(cap) = display_name_re.captures(&cfg_text).or_else(|| display_name_tag_re.captures(&cfg_text)) {
                    let name = cap[1].trim();
                    if !name.is_empty() && !name.starts_with("ms-resource:") {
                        resolved_name = Some(name.to_string());
                    }
                }
            }

            if resolved_poster.is_none() {
                if let Some(cap) = splash_re.captures(&cfg_text).or_else(|| logo_re.captures(&cfg_text)).or_else(|| store_logo_re.captures(&cfg_text)) {
                    let rel_img = cap[1].replace('/', "\\");
                    let img_path = dir.join(&rel_img);
                    if img_path.exists() {
                        resolved_poster = crate::core::steamart::file_to_art_uri(&img_path);
                    }
                }
            }

            if resolved_name.is_some() && resolved_poster.is_some() {
                break;
            }
        }
    }

    (resolved_name, resolved_poster)
}

pub fn discover_xbox() -> Vec<GameEntry> {
    let mut games = Vec::new();
    let mut candidate_dirs: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        // 1. Query HKLM\SOFTWARE\Microsoft\GamingServices\PackageRepository\Root
        let root_subs = win32_enum_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\GamingServices\PackageRepository\Root", KEY_READ);
        for sub in root_subs {
            let sub_path = format!(r"SOFTWARE\Microsoft\GamingServices\PackageRepository\Root\{}", sub);
            let child_subs = win32_enum_subkeys(HKEY_LOCAL_MACHINE, &sub_path, KEY_READ);
            for child in child_subs {
                let pkg_key = format!(r"{}\{}", sub_path, child);
                if let Some(raw_root) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &pkg_key, "Root", KEY_READ) {
                    let clean = raw_root.trim_start_matches(r"\\?\").trim_end_matches('\\').trim_end_matches('/');
                    let p = PathBuf::from(clean);
                    if p.exists() && p.is_dir() && !candidate_dirs.contains(&p) {
                        candidate_dirs.push(p);
                    }
                }
            }
        }
    }

    // 2. Scan standard XboxGames folders on all fixed drives
    for drive in get_fixed_drives() {
        let xgames = drive.join("XboxGames");
        if xgames.is_dir() {
            if let Ok(entries) = fs::read_dir(&xgames) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && !candidate_dirs.contains(&p) {
                        candidate_dirs.push(p);
                    }
                }
            }
        }
    }

    // 3. Scan candidate directories
    for dir in candidate_dirs {
        if let Some(mut game) = scan_game_directory(&dir) {
            game.launcher = "Xbox".to_string();

            let (xbox_name, xbox_poster) = extract_xbox_metadata(&dir);
            if let Some(name) = xbox_name {
                game.name = name;
            }
            if game.poster.is_none() {
                game.poster = xbox_poster;
            }

            games.push(game);
        }
    }

    games
}

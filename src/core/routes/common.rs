#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use crate::core::journal::{ActiveManifest, ManifestItem};

#[derive(Debug, Clone)]
pub struct DeployOptions {
    pub game_name: Option<String>,
    pub game_dir: PathBuf,
    pub exe_path: PathBuf,
    pub api: String,
    pub pre_sr: bool,
    pub passes: u32,
    pub mfg_unlock: bool,
    pub mfg_multiplier: u32,
    pub nr_style: usize,
    pub nr_style_enabled: bool,
}

impl Default for DeployOptions {
    fn default() -> Self {
        Self {
            game_name: None,
            game_dir: PathBuf::new(),
            exe_path: PathBuf::new(),
            api: "DirectX 12".to_string(),
            pre_sr: false,
            passes: 1,
            mfg_unlock: false,
            mfg_multiplier: 1,
            nr_style: 0,
            nr_style_enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeployResult {
    pub success: bool,
    pub log_lines: Vec<String>,
    pub replaced: usize,
    pub added: usize,
}

pub fn is_known_mod_file(dest: &Path) -> bool {
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if name == "optiscaler.ini"
        || name == "optiscaler.log"
        || name == "optiscaler.dll"
        || name == "reshade.ini"
        || name == "reshade.log"
        || name == "reshade64.dll"
        || name == "reshade32.dll"
        || name == "reshade64.json"
        || name == "reshadegui.ini"
        || name == "reshadepreset.ini"
        || name == "nvngx.dll_dlssnr.dll"
        || name == "nvngx_dlssnr.dll"
        || name == "dlss5-feed.cfg"
        || name == "dlss5-feed.log"
        || name == "dlss5-feed.addon64"
        || name == "dlss5-feed.addon32"
        || name == "dlss5-feed-host64.exe"
        || name == "dlss-overlay.addon64"
        || name == "dlss5-lab-overlay.addon64"
        || name == "renodx-dlss5.addon64"
        || name == "dlss-mip-fix.addon64"
        || name == "dlss-mip-fix.cfg"
        || name == "dlss-mip-fix.log"
        || name == "dlss5-d3d12-fix.addon64"
        || name == "dlss5-d3d12-fix.cfg"
        || name == "dlss5-d3d12-fix.log"
        || name == "renodx-mfgunlock.addon64"
        || name == "dgvoodoo.conf"
        || name == "dgvoodoo.log"
        || name == "rtxmfg-universal.json"
        || name == "rtx40mfg-universal.json"
        || name == "sl.pcl.dll"
        || name.starts_with("rtxmfg-")
        || name.ends_with(".addon64")
        || name.ends_with(".addon32")
        || name.ends_with(".addon")
    {
        return true;
    }
    if dest.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.eq_ignore_ascii_case("OptiScaler") || s.eq_ignore_ascii_case("host64") || s.eq_ignore_ascii_case("NativeMods")
    }) {
        return true;
    }
    let hook_names = ["dxgi.dll", "winmm.dll", "d3d12.dll", "d3d11.dll", "d3d9.dll", "d3d8.dll", "opengl32.dll", "dinput8.dll", "version.dll"];
    if hook_names.contains(&name.as_str()) && dest.is_file() {
        return crate::core::pe::is_optiscaler_or_proxy(dest) || crate::core::pe::is_reshade_dll(dest).0;
    }
    false
}

pub fn is_stale_proxy_dll(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if crate::core::pe::is_optiscaler_or_proxy(path) || crate::core::pe::is_reshade_dll(path).0 {
        return true;
    }
    if let Ok(bytes) = fs::read(path) {
        let s = String::from_utf8_lossy(&bytes).to_lowercase();
        if s.contains("reshade") || s.contains("optiscaler") || s.contains("dgvoodoo") {
            return true;
        }
    }
    false
}

pub fn remove_stale_proxy_hooks(mod_root: &Path, active_hooks: &[&str], log: &mut Vec<String>) {
    let check_hooks = ["winmm.dll", "version.dll", "dinput8.dll", "dxgi.dll", "d3d12.dll", "d3d11.dll", "d3d9.dll", "d3d8.dll", "opengl32.dll"];
    for stale_name in check_hooks {
        if !active_hooks.iter().any(|h| h.eq_ignore_ascii_case(stale_name)) {
            let stale_p = mod_root.join(stale_name);
            if is_stale_proxy_dll(&stale_p) {
                if fs::remove_file(&stale_p).is_ok() {
                    log.push(format!("[CLEANUP] Removed obsolete proxy hook: {}", stale_name));
                }
            }
        }
    }
}

pub fn clean_conflicting_route_artifacts(
    target_route: &str,
    game_dir: &Path,
    mod_root: &Path,
    mfg_unlock: bool,
    api: &str,
    log: &mut Vec<String>,
) -> std::io::Result<()> {
    let mut unique_dirs: Vec<PathBuf> = Vec::new();
    if mod_root.is_dir() {
        unique_dirs.push(mod_root.to_path_buf());
    }
    if game_dir.is_dir() && !unique_dirs.iter().any(|d| d == game_dir) {
        unique_dirs.push(game_dir.to_path_buf());
    }

    // Check if there is an existing active manifest for a DIFFERENT route
    if let Some(prev_manifest) = crate::core::journal::read_manifest(game_dir) {
        if prev_manifest.route != target_route {
            log.push(format!("[SWAP] Switching route from '{}' to '{}' - cleaning prior route artifacts", prev_manifest.route, target_route));
            for rel in &prev_manifest.added {
                let target_file = crate::core::journal::resolve_target_path(game_dir, rel);
                if target_file.is_file() {
                    let fname = target_file.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                    if is_known_mod_file(&target_file)
                        || fname.ends_with(".addon64")
                        || fname.ends_with(".addon32")
                        || fname.ends_with(".addon")
                        || fname.ends_with(".ini")
                        || fname.ends_with(".cfg")
                        || fname.ends_with(".log")
                    {
                        if fs::remove_file(&target_file).is_ok() {
                            log.push(format!("[SWAP] Purged prior route file: {}", rel));
                        }
                    }
                }
            }
            for rel_dir in prev_manifest.added_dirs.iter().rev() {
                let target_d = crate::core::journal::resolve_target_path(game_dir, rel_dir);
                if target_d.is_dir() {
                    let lower = rel_dir.to_lowercase();
                    if lower.contains("reshade") || lower.contains("optiscaler") || lower.contains("nativemods") {
                        if fs::remove_dir_all(&target_d).is_ok() {
                            log.push(format!("[SWAP] Purged prior route directory: {}", rel_dir));
                        }
                    }
                }
            }
        }
    }

    if target_route == "optiscaler" {
        // 1. Unregister Vulkan layer
        if crate::core::vulkan_layer::unregister_vulkan_layer(game_dir).unwrap_or(false) {
            log.push("[SWAP] Unregistered Vulkan implicit layer for OptiScaler deployment".to_string());
        }

        // 2. Remove ReShade, RenoDX, and Feeder files and directories
        for dir in &unique_dirs {
            let reshade_shaders = dir.join("reshade-shaders");
            if reshade_shaders.is_dir() {
                if fs::remove_dir_all(&reshade_shaders).is_ok() {
                    log.push(format!("[SWAP] Removed conflicting reshade-shaders/ from {}", dir.display()));
                }
            }

            let host64_dir = dir.join("host64");
            if host64_dir.is_dir() {
                if fs::remove_dir_all(&host64_dir).is_ok() {
                    log.push(format!("[SWAP] Removed conflicting host64/ from {}", dir.display()));
                }
            }

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        let lower = fname.to_lowercase();
                        let is_reshade_artifact = lower == "reshade.ini"
                            || lower == "reshadepreset.ini"
                            || lower == "reshadegui.ini"
                            || lower == "reshade.log"
                            || lower == "reshade64.dll"
                            || lower == "reshade32.dll"
                            || lower == "reshade64.json"
                            || lower == "dlss5-feed.cfg"
                            || lower == "dlss5-feed.log"
                            || lower == "dlss5-feed.addon64"
                            || lower == "dlss5-feed.addon32"
                            || lower == "dlss5-feed-host64.exe"
                            || lower == "dlss-overlay.addon64"
                            || lower == "dlss5-lab-overlay.addon64"
                            || lower == "renodx-dlss5.addon64"
                            || lower == "dlss-mip-fix.addon64"
                            || lower == "dlss-mip-fix.cfg"
                            || lower == "dlss-mip-fix.log"
                            || lower == "dlss5-d3d12-fix.addon64"
                            || lower == "dlss5-d3d12-fix.cfg"
                            || lower == "dlss5-d3d12-fix.log"
                            || lower == "renodx-mfgunlock.addon64"
                            || lower == "dgvoodoo.conf"
                            || lower == "dgvoodoo.log"
                            || lower.ends_with(".addon64")
                            || lower.ends_with(".addon32")
                            || lower.ends_with(".addon");

                        let is_reshade_hook = (lower == "dxgi.dll" || lower == "d3d11.dll" || lower == "d3d12.dll" || lower == "d3d9.dll" || lower == "d3d8.dll" || lower == "opengl32.dll")
                            && (crate::core::pe::is_reshade_dll(&path).0 || crate::core::pe::is_optiscaler_or_proxy(&path) || {
                                if let Ok(bytes) = fs::read(&path) {
                                    let s = String::from_utf8_lossy(&bytes).to_lowercase();
                                    s.contains("reshade") || s.contains("dgvoodoo")
                                } else {
                                    false
                                }
                            });

                        if is_reshade_artifact || is_reshade_hook {
                            if fs::remove_file(&path).is_ok() {
                                log.push(format!("[SWAP] Cleaned conflicting ReShade file: {}", fname));
                            }
                        }
                    }
                }
            }

            // Also clean standalone MFG artifacts if mfg_unlock is false
            if !mfg_unlock {
                let ver_p = dir.join("version.dll");
                if ver_p.is_file() && (is_stale_proxy_dll(&ver_p) || crate::core::pe::is_optiscaler_or_proxy(&ver_p)) {
                    if fs::remove_file(&ver_p).is_ok() {
                        log.push("[CLEANUP] Removed standalone MFG version.dll (MFG disabled)".to_string());
                    }
                }
                let _ = fs::remove_file(dir.join("RTXMFG-Universal.json"));
                let _ = fs::remove_file(dir.join("RTX40MFG-Universal.json"));
            }
        }
    } else {
        // target_route is "feeder" or "native"
        // 1. Purge OptiScaler files and directories
        for dir in &unique_dirs {
            let opti_dir = dir.join("OptiScaler");
            if opti_dir.is_dir() {
                if fs::remove_dir_all(&opti_dir).is_ok() {
                    log.push(format!("[SWAP] Removed conflicting OptiScaler/ directory from {}", dir.display()));
                }
            }

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        let lower = fname.to_lowercase();
                        let is_optiscaler_artifact = lower == "optiscaler.ini"
                            || lower == "optiscaler.log"
                            || lower == "optiscaler.dll"
                            || lower == "nvngx.dll_dlssnr.dll"
                            || lower == "nvngx_dlssnr.dll"
                            || lower == "rtxmfg-universal.json"
                            || lower == "rtx40mfg-universal.json"
                            || (lower.starts_with("rtxmfg-") && lower.ends_with(".json"));

                        let is_opti_or_mfg_hook = (lower == "dxgi.dll" || lower == "version.dll" || lower == "d3d12.dll" || lower == "d3d11.dll" || lower == "d3d9.dll" || lower == "d3d8.dll")
                            && (crate::core::pe::is_optiscaler_or_proxy(&path) || {
                                if let Ok(bytes) = fs::read(&path) {
                                    let s = String::from_utf8_lossy(&bytes).to_lowercase();
                                    s.contains("optiscaler") || s.contains("rtxmfg") || s.contains("dgvoodoo")
                                } else {
                                    false
                                }
                            });

                        if is_optiscaler_artifact || is_opti_or_mfg_hook {
                            if fs::remove_file(&path).is_ok() {
                                log.push(format!("[SWAP] Cleaned conflicting OptiScaler file: {}", fname));
                            }
                        }
                    }
                }
            }

            if target_route == "native" {
                let _ = fs::remove_file(dir.join("dlss5-feed.cfg"));
                let _ = fs::remove_file(dir.join("dlss5-feed.log"));
                let _ = fs::remove_file(dir.join("dlss5-feed.addon64"));
                let _ = fs::remove_file(dir.join("dlss5-feed.addon32"));
                let _ = fs::remove_file(dir.join("dlss5-feed-host64.exe"));
                let _ = fs::remove_file(dir.join("dgvoodoo.conf"));
                let _ = fs::remove_file(dir.join("dgvoodoo.log"));
                let _ = fs::remove_file(dir.join("ReShadePreset.ini"));
                let reshade_shaders = dir.join("reshade-shaders");
                if reshade_shaders.is_dir() {
                    let _ = fs::remove_dir_all(&reshade_shaders);
                }
                let host64_dir = dir.join("host64");
                if host64_dir.is_dir() {
                    let _ = fs::remove_dir_all(&host64_dir);
                }
            } else if target_route == "feeder" {
                let _ = fs::remove_file(dir.join("dlss-overlay.addon64"));
                let _ = fs::remove_file(dir.join("dlss5-lab-overlay.addon64"));
            }
        }

        let has_vulkan_target = api.to_lowercase().contains("vulkan")
            || mod_root.read_dir().map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    let p = e.path();
                    p.is_file()
                        && p.extension().map(|ext| ext.eq_ignore_ascii_case("exe")).unwrap_or(false)
                        && crate::core::scan::detect_api_for_exe(&p).map(|a| a.to_lowercase().contains("vulkan")).unwrap_or(false)
                })
            }).unwrap_or(false);

        if !has_vulkan_target {
            let _ = crate::core::vulkan_layer::unregister_vulkan_layer(game_dir);
        }
    }

    Ok(())
}

pub fn carry_forward_existing_backups(
    game_dir: &Path,
    backup_dir: &Path,
    manifest: &mut ActiveManifest,
    log: &mut Vec<String>,
) {
    if let Some(prev) = crate::core::journal::read_manifest(game_dir) {
        let prev_bdir = crate::core::journal::backup_dir(game_dir);
        for item in prev.replaced {
            let old_backup_file = if let Some(ref p) = prev.backup_prefix {
                prev_bdir.join(p).join(&item.rel)
            } else {
                prev_bdir.join(&item.rel)
            };
            if old_backup_file.is_file() && !crate::core::journal::is_corrupted_or_mod_backup(&old_backup_file) {
                let new_backup_file = backup_dir.join(&item.rel);
                if let Some(parent) = new_backup_file.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if fs::copy(&old_backup_file, &new_backup_file).is_ok() {
                    if !manifest.replaced.iter().any(|r| r.rel == item.rel) {
                        manifest.replaced.push(item.clone());
                        log.push(format!("[BACKUP] Preserved original vanilla backup: {}", item.rel));
                    }
                }
            }
        }
    }
}

pub fn track_and_copy(
    manifest: &mut ActiveManifest,
    game_dir: &Path,
    backup_dir: &Path,
    src: &Path,
    dest: &Path,
    kind: &str,
    log: &mut Vec<String>,
) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
            let rel_p = parent.strip_prefix(game_dir).unwrap_or(parent).to_string_lossy().to_string();
            if !rel_p.is_empty() && !manifest.added_dirs.contains(&rel_p) {
                manifest.added_dirs.push(rel_p);
            }
        }
    }

    let rel = dest.strip_prefix(game_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dest.file_name().unwrap_or_default().to_string_lossy().to_string());

    if dest.exists() {
        let is_same_as_src = if let (Ok(d_meta), Ok(s_meta)) = (dest.metadata(), src.metadata()) {
            if d_meta.len() == s_meta.len() {
                fs::read(dest).ok() == fs::read(src).ok()
            } else {
                false
            }
        } else {
            false
        };

        let was_previously_added = if let Some(prev) = crate::core::journal::read_manifest(game_dir) {
            prev.added.contains(&rel)
        } else {
            false
        };

        if is_known_mod_file(dest) || is_same_as_src || was_previously_added {
            if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
                manifest.added.push(rel.clone());
            }
        } else if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            let backup_dest = backup_dir.join(&rel);
            if let Some(p) = backup_dest.parent() {
                fs::create_dir_all(p)?;
            }
            let _ = fs::copy(dest, &backup_dest);
            manifest.replaced.push(ManifestItem {
                rel: rel.clone(),
                old_hash: None,
                kind: Some(kind.to_string()),
            });
            log.push(format!("@{{log_backup_saved|{}}}", rel));
        }
    } else {
        if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            manifest.added.push(rel.clone());
        }
    }

    fs::copy(src, dest)?;
    log.push(format!("@{{log_copy_deployed|{}}}", rel));
    Ok(())
}

pub fn track_and_write(
    manifest: &mut ActiveManifest,
    game_dir: &Path,
    backup_dir: &Path,
    dest: &Path,
    content: &str,
    kind: &str,
    log: &mut Vec<String>,
) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
            let rel_p = parent.strip_prefix(game_dir).unwrap_or(parent).to_string_lossy().to_string();
            if !rel_p.is_empty() && !manifest.added_dirs.contains(&rel_p) {
                manifest.added_dirs.push(rel_p);
            }
        }
    }

    let rel = dest.strip_prefix(game_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dest.file_name().unwrap_or_default().to_string_lossy().to_string());

    if dest.exists() {
        if is_known_mod_file(dest) {
            if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
                manifest.added.push(rel.clone());
            }
        } else if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            let backup_dest = backup_dir.join(&rel);
            if let Some(p) = backup_dest.parent() {
                fs::create_dir_all(p)?;
            }
            let _ = fs::copy(dest, &backup_dest);
            manifest.replaced.push(ManifestItem {
                rel: rel.clone(),
                old_hash: None,
                kind: Some(kind.to_string()),
            });
            log.push(format!("@{{log_backup_saved|{}}}", rel));
        }
    } else {
        if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            manifest.added.push(rel.clone());
        }
    }

    fs::write(dest, content)?;
    log.push(format!("@{{log_config_written|{}}}", rel));
    Ok(())
}

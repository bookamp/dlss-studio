use std::fs;
use std::path::Path;
use super::manifest::{
    backup_dir, prune_old_manifests, read_latest_done_manifest, read_manifest,
    read_or_backfill_added_history, resolve_target_path, strip_verbatim,
};

pub fn is_proxy_hook(path: &Path) -> bool {
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    let hook_names = ["dxgi.dll", "winmm.dll", "d3d12.dll", "d3d11.dll", "d3d9.dll", "d3d8.dll", "opengl32.dll", "dinput8.dll", "version.dll"];
    if hook_names.contains(&fname.as_str()) {
        return crate::core::pe::is_optiscaler_or_proxy(path) || crate::core::pe::is_reshade_dll(path).0;
    }
    false
}

pub fn is_corrupted_or_mod_backup(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if is_proxy_hook(path) {
        return true;
    }
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if fname == "optiscaler.ini"
        || fname == "optiscaler.log"
        || fname == "optiscaler.dll"
        || fname == "reshade.ini"
        || fname == "reshade.log"
        || fname == "reshade64.dll"
        || fname == "reshade32.dll"
        || fname == "reshade64.json"
        || fname == "reshadegui.ini"
        || fname == "reshadepreset.ini"
        || fname == "nvngx.dll_dlssnr.dll"
        || fname == "nvngx_dlssnr.dll"
        || fname == "dlss5-feed.cfg"
        || fname == "dlss5-feed.log"
        || fname == "dlss5-feed.addon64"
        || fname == "dlss5-feed.addon32"
        || fname == "dlss5-feed-host64.exe"
        || fname == "dlss-overlay.addon64"
        || fname == "dlss5-lab-overlay.addon64"
        || fname == "renodx-dlss5.addon64"
        || fname == "dlss-mip-fix.addon64"
        || fname == "dlss-mip-fix.cfg"
        || fname == "dlss-mip-fix.log"
        || fname == "dlss5-d3d12-fix.addon64"
        || fname == "dlss5-d3d12-fix.cfg"
        || fname == "dlss5-d3d12-fix.log"
        || fname == "renodx-mfgunlock.addon64"
        || fname == "dgvoodoo.conf"
        || fname == "dgvoodoo.log"
        || fname == "rtxmfg-universal.json"
        || fname == "rtx40mfg-universal.json"
        || fname == "sl.pcl.dll"
        || fname.starts_with("rtxmfg-")
        || fname.ends_with(".addon64")
        || fname.ends_with(".addon32")
        || fname.ends_with(".addon")
    {
        return true;
    }
    false
}

pub fn archive_added_history(game_dir: &Path) -> std::io::Result<()> {
    let bdir = backup_dir(game_dir);
    let hist_path = bdir.join("added_history.json");
    if hist_path.exists() {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let archive_name = format!("added_history.json.done-{}", ts);
        let _ = fs::rename(&hist_path, bdir.join(&archive_name));
        crate::core::logger::info("journal", &format!("Archived cumulative added history to {}", archive_name));
    }
    Ok(())
}

pub fn clean_untracked_mods(game_dir: &Path) -> std::io::Result<Vec<String>> {
    clean_untracked_mods_with_exe(game_dir, None)
}

pub fn clean_untracked_mods_with_exe(game_dir: &Path, exe_path: Option<&Path>) -> std::io::Result<Vec<String>> {
    let mut removed = Vec::new();
    let mut target_dirs = vec![game_dir.to_path_buf()];

    let detected_exe = if exe_path.is_none() {
        crate::core::scan::scan_game_directory(game_dir).map(|g| g.exe_path)
    } else {
        None
    };
    let effective_exe = exe_path.or(detected_exe.as_deref());

    if let Some(ep) = effective_exe {
        if let Some(parent) = ep.parent() {
            target_dirs.push(parent.to_path_buf());
        }
    }
    if let Some(mod_root) = crate::core::compatibility::managed_mod_root(game_dir, effective_exe) {
        target_dirs.push(mod_root);
    }

    if let Err(e) = crate::core::install_guards::assert_game_closed(game_dir, effective_exe) {
        crate::core::logger::warn("journal", &format!("Cannot clean untracked mods while game is running: {}", e));
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e));
    }

    if let Ok(canon) = fs::canonicalize(game_dir) {
        let norm = strip_verbatim(&canon);
        if norm != game_dir {
            target_dirs.push(norm.clone());
        }
        if let Some(parent) = norm.parent() {
            let fname = norm.file_name().and_then(|f| f.to_str()).unwrap_or("");
            if fname.eq_ignore_ascii_case("content") || fname.eq_ignore_ascii_case("win64") || fname.eq_ignore_ascii_case("binaries") {
                target_dirs.push(parent.to_path_buf());
            }
        }
    }
    if game_dir.join("Content").is_dir() {
        target_dirs.push(game_dir.join("Content"));
    }

    // Recursively discover subdirectories up to depth 4 to catch nested executable folders (e.g. bin\x64, Binaries\Win64)
    for entry in walkdir::WalkDir::new(game_dir).max_depth(4).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_dir() {
            let fname = entry.file_name().to_string_lossy();
            if !fname.starts_with('.') && fname != "_DLSS5_Backup" && fname != "node_modules" {
                target_dirs.push(entry.path().to_path_buf());
            }
        }
    }

    // Deduplicate target dirs
    let mut unique_dirs = Vec::new();
    for d in target_dirs {
        if !unique_dirs.contains(&d) && d.is_dir() {
            unique_dirs.push(d);
        }
    }

    // Also purge files listed in cumulative added history, active manifest, or recent manifest
    let history = read_or_backfill_added_history(game_dir);
    for rel in &history.files {
        let target_file = resolve_target_path(game_dir, rel);
        if target_file.exists() && fs::remove_file(&target_file).is_ok() {
            removed.push(rel.clone());
        }
    }
    for rel in history.dirs.iter().rev() {
        let target_dir = resolve_target_path(game_dir, rel);
        if target_dir.exists() && fs::remove_dir_all(&target_dir).is_ok() {
            removed.push(format!("{}/", rel));
        }
    }

    if let Some(manifest) = read_manifest(game_dir).or_else(|| read_latest_done_manifest(game_dir)) {
        for rel in &manifest.added {
            let target_file = resolve_target_path(game_dir, rel);
            if target_file.exists() && fs::remove_file(&target_file).is_ok() {
                if !removed.contains(rel) {
                    removed.push(rel.clone());
                }
            }
        }
        for rel in manifest.added_dirs.iter().rev() {
            let target_dir = resolve_target_path(game_dir, rel);
            if target_dir.exists() && fs::remove_dir_all(&target_dir).is_ok() {
                let formatted = format!("{}/", rel);
                if !removed.contains(&formatted) {
                    removed.push(formatted);
                }
            }
        }
    }

    for dir in unique_dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let fname = entry.file_name().to_string_lossy().to_string();
                let lower = fname.to_lowercase();

                if lower == "optiscaler.ini"
                    || lower == "optiscaler.log"
                    || lower == "optiscaler.dll"
                    || lower == "reshade.ini"
                    || lower == "reshade.log"
                    || lower == "reshade64.dll"
                    || lower == "reshade32.dll"
                    || lower == "reshade64.json"
                    || lower == "reshadegui.ini"
                    || lower == "reshadepreset.ini"
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
                    || lower == "nvngx.dll_dlssnr.dll"
                    || lower == "nvngx_dlssnr.dll"
                    || lower == "rtxmfg-universal.json"
                    || lower == "rtx40mfg-universal.json"
                    || lower == "sl.pcl.dll"
                    || lower.starts_with("rtxmfg-")
                    || lower.ends_with(".addon64")
                    || lower.ends_with(".addon32")
                    || lower.ends_with(".addon")
                {
                    match fs::remove_file(&path) {
                        Ok(_) => {
                            removed.push(fname.clone());
                        }
                        Err(e) => {
                            crate::core::logger::warn("journal", &format!("Failed to remove mod file {}: {}", path.display(), e));
                            return Err(std::io::Error::new(e.kind(), format!("Failed to remove {}: {}. Is the game or launcher still running?", fname, e)));
                        }
                    }
                } else if (lower == "optiscaler" || lower == "host64") && path.is_dir() {
                    if let Err(e) = fs::remove_dir_all(&path) {
                        return Err(std::io::Error::new(e.kind(), format!("Failed to remove {} directory: {}. Is the game running?", fname, e)));
                    }
                    removed.push(format!("{}/", fname));
                } else if lower == "reshade-shaders" && path.is_dir() {
                    if let Err(e) = fs::remove_dir_all(&path) {
                        return Err(std::io::Error::new(e.kind(), format!("Failed to remove reshade-shaders directory: {}. Is the game running?", e)));
                    }
                    removed.push("reshade-shaders/".to_string());
                } else if lower == "dxgi.dll" || lower == "winmm.dll" || lower == "d3d12.dll" || lower == "d3d11.dll" || lower == "d3d9.dll" || lower == "d3d8.dll" || lower == "dinput8.dll" || lower == "version.dll" {
                    if is_proxy_hook(&path) {
                        match fs::remove_file(&path) {
                            Ok(_) => {
                                removed.push(fname.clone());
                            }
                            Err(e) => {
                                crate::core::logger::warn("journal", &format!("Failed to remove proxy hook {}: {}", path.display(), e));
                                return Err(std::io::Error::new(e.kind(), format!("Failed to remove {}: {}. Is the game or launcher still running?", fname, e)));
                            }
                        }
                    }
                }
            }
        }
    }

    // If active manifest exists, archive it so it doesn't leave active state
    let bdir = backup_dir(game_dir);
    let manifest_path = bdir.join("manifest.json");
    if manifest_path.exists() {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let archive_name = format!("manifest.json.done-{}", ts);
        let _ = fs::rename(&manifest_path, bdir.join(&archive_name));
        let _ = prune_old_manifests(game_dir, 5);
    }

    if !removed.is_empty() {
        crate::core::logger::info("journal", &format!("Cleaned untracked mod files for {}: {:?}", game_dir.display(), removed));
    }

    Ok(removed)
}

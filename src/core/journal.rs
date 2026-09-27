use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestItem {
    pub rel: String,
    #[serde(default)]
    pub old_hash: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestGame {
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub exe: Option<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub bitness: Option<u32>,
    #[serde(default)]
    pub api_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveManifest {
    #[serde(default = "default_manifest_version")]
    pub version: u32,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub route: String,
    #[serde(default)]
    pub game: Option<ManifestGame>,
    #[serde(default)]
    pub game_exe: Option<String>,
    #[serde(default)]
    pub backup_prefix: Option<String>,
    #[serde(default)]
    pub replaced: Vec<ManifestItem>,
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub added_dirs: Vec<String>,
    #[serde(default)]
    pub mfg_unlock: Option<bool>,
    #[serde(default)]
    pub mfg_multiplier: Option<u32>,
    #[serde(default)]
    pub nr_style_enabled: Option<bool>,
    #[serde(default)]
    pub nr_style: Option<usize>,
    #[serde(default)]
    pub opti_presr: Option<bool>,
    #[serde(default)]
    pub opti_passes: Option<u32>,
}

fn default_manifest_version() -> u32 { 1 }

impl Default for ActiveManifest {
    fn default() -> Self {
        Self {
            version: 1,
            date: format!("{:?}", std::time::SystemTime::now()),
            route: "optiscaler".to_string(),
            game: None,
            game_exe: Some(String::new()),
            backup_prefix: None,
            replaced: Vec::new(),
            added: Vec::new(),
            added_dirs: Vec::new(),
            mfg_unlock: None,
            mfg_multiplier: None,
            nr_style_enabled: None,
            nr_style: None,
            opti_presr: None,
            opti_passes: None,
        }
    }
}

fn strip_verbatim(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

pub fn backup_dir(game_dir: &Path) -> PathBuf {
    // 1. Direct subfolder
    let direct = game_dir.join("_DLSS5_Backup");
    if direct.exists() {
        return direct;
    }

    // 2. Canonicalized path (resolving NTFS Junctions / symlinks like Xbox WindowsApps -> Games\...\Content)
    if let Ok(canon) = fs::canonicalize(game_dir) {
        let norm = strip_verbatim(&canon);
        let c_direct = norm.join("_DLSS5_Backup");
        if c_direct.exists() {
            return c_direct;
        }
        if let Some(parent) = norm.parent() {
            let p_backup = parent.join("_DLSS5_Backup");
            if p_backup.exists() {
                return p_backup;
            }
        }
    }

    // 3. Parent of game_dir if game_dir is nested e.g. in "Content"
    if let Some(parent) = game_dir.parent() {
        let p_backup = parent.join("_DLSS5_Backup");
        if p_backup.exists() {
            return p_backup;
        }
    }

    direct
}

pub fn has_backup_available(game_dir: &Path) -> bool {
    let bdir = backup_dir(game_dir);
    bdir.join("manifest.json").exists() || bdir.join("pending-switch.json").exists()
}

pub fn read_manifest(game_dir: &Path) -> Option<ActiveManifest> {
    let bdir = backup_dir(game_dir);
    let manifest_file = bdir.join("manifest.json");
    if let Ok(bytes) = fs::read(&manifest_file) {
        if let Ok(m) = serde_json::from_slice::<ActiveManifest>(&bytes) {
            return Some(m);
        }
    }
    None
}

pub fn read_latest_done_manifest(game_dir: &Path) -> Option<ActiveManifest> {
    let bdir = backup_dir(game_dir);
    if let Ok(entries) = fs::read_dir(&bdir) {
        let mut done_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("manifest.json.done-"))
                    .unwrap_or(false)
            })
            .collect();

        // Sort so latest timestamp is first
        done_files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

        for p in done_files {
            if let Ok(bytes) = fs::read(&p) {
                if let Ok(m) = serde_json::from_slice::<ActiveManifest>(&bytes) {
                    return Some(m);
                }
            }
        }
    }

    None
}


pub fn prune_old_manifests(game_dir: &Path, max_keep: usize) -> usize {
    let bdir = backup_dir(game_dir);
    if !bdir.exists() {
        return 0;
    }

    let entries = match fs::read_dir(&bdir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut done_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("manifest.json.done-"))
                .unwrap_or(false)
        })
        .collect();

    // Sort descending by file name (which includes epoch timestamp)
    done_files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

    // Collect all referenced backup_prefix strings from retained / active manifests
    let mut referenced_prefixes = std::collections::HashSet::new();

    // 1. Active manifest
    if let Some(active) = read_manifest(game_dir) {
        if let Some(prefix) = active.backup_prefix {
            referenced_prefixes.insert(prefix);
        }
    }

    // 2. Pending switch manifest (if any)
    let pending_path = bdir.join("pending-switch.json");
    if let Ok(bytes) = fs::read(&pending_path) {
        if let Ok(pending) = serde_json::from_slice::<ActiveManifest>(&bytes) {
            if let Some(prefix) = pending.backup_prefix {
                referenced_prefixes.insert(prefix);
            }
        }
    }

    // 3. Top `max_keep` retained done manifests
    let retained_count = done_files.len().min(max_keep);
    for p in &done_files[..retained_count] {
        if let Ok(bytes) = fs::read(p) {
            if let Ok(m) = serde_json::from_slice::<ActiveManifest>(&bytes) {
                if let Some(prefix) = m.backup_prefix {
                    referenced_prefixes.insert(prefix);
                }
            }
        }
    }

    let mut pruned_count = 0;

    // Prune excess done manifests
    if done_files.len() > max_keep {
        for p in &done_files[max_keep..] {
            let prefix_opt = if let Ok(bytes) = fs::read(p) {
                serde_json::from_slice::<ActiveManifest>(&bytes)
                    .ok()
                    .and_then(|m| m.backup_prefix)
            } else {
                None
            };

            if fs::remove_file(p).is_ok() {
                pruned_count += 1;
                crate::core::logger::debug("journal", &format!("Pruned old archived manifest: {}", p.display()));

                // If this manifest had a backup_prefix that is NOT referenced anywhere else, clean it up
                if let Some(prefix) = prefix_opt {
                    if !referenced_prefixes.contains(&prefix) {
                        let prefix_path = bdir.join(&prefix);
                        if prefix_path.is_dir() {
                            let _ = fs::remove_dir_all(&prefix_path);
                            crate::core::logger::info("journal", &format!("Cleaned orphaned backup directory: {}", prefix_path.display()));
                        }
                    }
                }
            }
        }
    }

    // Also check the "originals" folder directly for any orphaned subdirectories
    let originals_dir = bdir.join("originals");
    if originals_dir.is_dir() {
        if let Ok(orig_entries) = fs::read_dir(&originals_dir) {
            for entry in orig_entries.flatten() {
                let sub_path = entry.path();
                if sub_path.is_dir() {
                    let rel_prefix = format!("originals/{}", entry.file_name().to_string_lossy());
                    if !referenced_prefixes.contains(&rel_prefix) {
                        let _ = fs::remove_dir_all(&sub_path);
                        crate::core::logger::info("journal", &format!("Cleaned unreferenced originals directory: {}", sub_path.display()));
                    }
                }
            }
        }
    }

    if pruned_count > 0 {
        crate::core::logger::info("journal", &format!("Pruned {} old archived manifests (retained top {}) for {}", pruned_count, max_keep, game_dir.display()));
    }

    pruned_count
}

pub fn save_manifest(game_dir: &Path, manifest: &ActiveManifest) -> std::io::Result<()> {
    crate::core::logger::info("journal", &format!("Saving active manifest for {}: route={}, replaced={}, added={}", game_dir.display(), manifest.route, manifest.replaced.len(), manifest.added.len()));
    let bdir = backup_dir(game_dir);
    fs::create_dir_all(&bdir)?;
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(bdir.join("manifest.json"), bytes)?;
    let _ = prune_old_manifests(game_dir, 5);
    Ok(())
}

pub fn resolve_target_path(game_dir: &Path, rel: &str) -> PathBuf {
    let direct = game_dir.join(rel);
    if direct.exists() {
        return direct;
    }
    // Check stripped Content prefix if game_dir is already Content or a junction to Content
    if let Ok(stripped) = Path::new(rel).strip_prefix("Content") {
        let sub = game_dir.join(stripped);
        if sub.exists() {
            return sub;
        }
    }
    if let Ok(stripped) = Path::new(rel).strip_prefix("content") {
        let sub = game_dir.join(stripped);
        if sub.exists() {
            return sub;
        }
    }
    // Check game_dir.join("Content").join(rel)
    let c_sub = game_dir.join("Content").join(rel);
    if c_sub.exists() {
        return c_sub;
    }

    if let Ok(canon) = fs::canonicalize(game_dir) {
        let norm = strip_verbatim(&canon);
        let c_direct = norm.join(rel);
        if c_direct.exists() {
            return c_direct;
        }
        if let Some(parent) = norm.parent() {
            let p_direct = parent.join(rel);
            if p_direct.exists() {
                return p_direct;
            }
        }
    }

    if let Ok(stripped) = Path::new(rel).strip_prefix("Content") {
        game_dir.join(stripped)
    } else {
        direct
    }
}

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

    // Also purge files listed in any active or recent manifest
    if let Some(manifest) = read_manifest(game_dir).or_else(|| read_latest_done_manifest(game_dir)) {
        for rel in &manifest.added {
            let target_file = resolve_target_path(game_dir, rel);
            if target_file.exists() && fs::remove_file(&target_file).is_ok() {
                removed.push(rel.clone());
            }
        }
        for rel in manifest.added_dirs.iter().rev() {
            let target_dir = resolve_target_path(game_dir, rel);
            if target_dir.exists() && fs::remove_dir_all(&target_dir).is_ok() {
                removed.push(format!("{}/", rel));
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

pub fn restore_game(game_dir: &Path) -> std::io::Result<bool> {
    crate::core::logger::info("restore", &format!("Initiating restore for game: {}", game_dir.display()));
    let manifest = match read_manifest(game_dir) {
        Some(m) => m,
        None => {
            // Fallback: clean untracked mods if no manifest is found
            let cleaned = clean_untracked_mods(game_dir)?;
            crate::core::logger::info("restore", &format!("No active manifest found for {}; cleaned untracked mods: {:?}", game_dir.display(), cleaned));
            return Ok(!cleaned.is_empty());
        }
    };

    let exe_opt = manifest.game_exe.as_ref()
        .or_else(|| manifest.game.as_ref().and_then(|g| g.exe.as_ref()))
        .map(|e| game_dir.join(e));

    if let Err(e) = crate::core::install_guards::assert_game_closed(game_dir, exe_opt.as_deref()) {
        crate::core::logger::warn("restore", &format!("Cannot restore while game is running: {}", e));
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e));
    }

    let bdir = backup_dir(game_dir);

    for item in &manifest.replaced {
        let backup_file = if let Some(ref p) = manifest.backup_prefix {
            bdir.join(p).join(&item.rel)
        } else {
            bdir.join(&item.rel)
        };
        let target_file = resolve_target_path(game_dir, &item.rel);
        if backup_file.exists() {
            // Safety check: if the backed-up file was actually a proxy hook or mod file
            // that was mistakenly copied to backup during a prior dirty install, NEVER restore it!
            if is_corrupted_or_mod_backup(&backup_file) {
                crate::core::logger::warn("restore", &format!("Skipping corrupted backup file (is proxy hook or mod): {}", backup_file.display()));
                if target_file.exists() {
                    let _ = fs::remove_file(&target_file);
                }
                continue;
            }

            if let Some(parent) = target_file.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&backup_file, &target_file)?;
            crate::core::logger::debug("restore", &format!("Restored original file: {}", target_file.display()));
        }
    }

    for rel in &manifest.added {
        let target_file = resolve_target_path(game_dir, rel);
        if target_file.exists() {
            let _ = fs::remove_file(&target_file);
            crate::core::logger::debug("restore", &format!("Removed mod file: {}", target_file.display()));
        }
    }

    for rel in manifest.added_dirs.iter().rev() {
        let target_dir = resolve_target_path(game_dir, rel);
        if target_dir.exists() {
            let _ = fs::remove_dir_all(&target_dir);
            crate::core::logger::debug("restore", &format!("Removed mod directory: {}", target_dir.display()));
        }
    }

    // Inspect all historical manifests and remove any file that was ever recorded in added
    if let Ok(entries) = fs::read_dir(&bdir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with("manifest.json.done-") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(past_m) = serde_json::from_str::<ActiveManifest>(&content) {
                        for rel in &past_m.added {
                            let target_file = resolve_target_path(game_dir, rel);
                            if target_file.exists() {
                                let _ = fs::remove_file(&target_file);
                                crate::core::logger::debug("restore", &format!("Removed historically added file: {}", target_file.display()));
                            }
                        }
                        for rel in past_m.added_dirs.iter().rev() {
                            let target_dir = resolve_target_path(game_dir, rel);
                            if target_dir.exists() {
                                let _ = fs::remove_dir_all(&target_dir);
                            }
                        }
                    }
                }
            }
        }
    }

    // Also purge known leftover injected mod files
    let exe_opt = manifest.game_exe.as_ref()
        .or_else(|| manifest.game.as_ref().and_then(|g| g.exe.as_ref()))
        .map(|e| game_dir.join(e));
    let _ = clean_untracked_mods_with_exe(game_dir, exe_opt.as_deref());
    let _ = crate::core::vulkan_layer::unregister_vulkan_layer(game_dir);

    let manifest_path = bdir.join("manifest.json");
    if manifest_path.exists() {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
        let archive_name = format!("manifest.json.done-{}", ts);
        let _ = fs::rename(&manifest_path, bdir.join(&archive_name));
        crate::core::logger::info("restore", &format!("Archived manifest to: {}", archive_name));
        let _ = prune_old_manifests(game_dir, 5);
    }

    crate::core::logger::info("restore", &format!("Restore successfully completed for {}", game_dir.display()));

    Ok(true)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HistoryRow {
    pub date: String,
    pub dir: String,
    #[serde(default)]
    pub game_name: Option<String>,
    pub action: String,
    pub replaced: usize,
    pub added: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryFile {
    pub version: String,
    pub entries: Vec<HistoryRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum HistoryStorageFormat {
    Versioned(HistoryFile),
    Legacy(Vec<HistoryRow>),
}

pub fn history_path() -> PathBuf {
    crate::core::state::get_appdata_dir().join("history.json")
}

pub fn save_history_file(file: &HistoryFile) -> std::io::Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(file)?;
    fs::write(path, bytes)?;
    Ok(())
}

pub fn parse_history_bytes(bytes: &[u8], current_version: &str) -> (Vec<HistoryRow>, Option<HistoryFile>) {
    if let Ok(format) = serde_json::from_slice::<HistoryStorageFormat>(bytes) {
        match format {
            HistoryStorageFormat::Versioned(file) => {
                let filtered: Vec<HistoryRow> = file.entries.into_iter()
                    .filter(|r| !r.dir.contains("dlss_test_") && !r.dir.contains("dlss_addon_test_"))
                    .collect();
                if file.version != current_version {
                    crate::core::logger::info("journal", &format!("Migrating history.json from {} to {}", file.version, current_version));
                    let upgraded = HistoryFile {
                        version: current_version.to_string(),
                        entries: filtered.clone(),
                    };
                    return (filtered, Some(upgraded));
                }
                return (filtered, None);
            }
            HistoryStorageFormat::Legacy(rows) => {
                let filtered: Vec<HistoryRow> = rows.into_iter()
                    .filter(|r| !r.dir.contains("dlss_test_") && !r.dir.contains("dlss_addon_test_"))
                    .collect();
                crate::core::logger::info("journal", &format!("Migrating legacy history.json to version {}", current_version));
                let upgraded = HistoryFile {
                    version: current_version.to_string(),
                    entries: filtered.clone(),
                };
                return (filtered, Some(upgraded));
            }
        }
    }
    (Vec::new(), None)
}

pub fn read_history() -> Vec<HistoryRow> {
    let path = history_path();
    if let Ok(bytes) = fs::read(&path) {
        let (rows, migration) = parse_history_bytes(&bytes, env!("CARGO_PKG_VERSION"));
        if let Some(upgraded) = migration {
            let _ = save_history_file(&upgraded);
        }
        return rows;
    }
    Vec::new()
}

pub fn append_history(row: &HistoryRow) -> std::io::Result<()> {
    if row.dir.contains("dlss_test_") || row.dir.contains("dlss_addon_test_") || row.dir.contains("test_") {
        return Ok(());
    }
    let mut history = read_history();
    history.push(row.clone());
    let file = HistoryFile {
        version: env!("CARGO_PKG_VERSION").to_string(),
        entries: history,
    };
    save_history_file(&file)?;
    Ok(())
}

pub fn now_timestamp_str() -> String {
    let now = std::time::SystemTime::now();
    if let Ok(dur) = now.duration_since(std::time::UNIX_EPOCH) {
        let secs = dur.as_secs();
        let days = secs / 86400;
        let day_secs = secs % 86400;
        let hours = day_secs / 3600;
        let mins = (day_secs % 3600) / 60;
        let mut y = 1970;
        let mut d = days;
        loop {
            let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
            let ydays = if leap { 366 } else { 365 };
            if d < ydays { break; }
            d -= ydays;
            y += 1;
        }
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut m = 1;
        for (idx, &dim) in mdays.iter().enumerate() {
            if d < dim { m = idx + 1; break; }
            d -= dim;
        }
        format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", y, m, d + 1, hours, mins)
    } else {
        "Recently".to_string()
    }
}





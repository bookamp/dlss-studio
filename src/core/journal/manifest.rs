use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddedHistory {
    #[serde(default)]
    pub files: BTreeSet<String>,
    #[serde(default)]
    pub dirs: BTreeSet<String>,
}

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

fn default_manifest_version() -> u32 {
    1
}

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

pub fn strip_verbatim(path: &Path) -> PathBuf {
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

    // 2. Ancestor of game_dir if game_dir is deeply nested e.g. in "Content/NewMoon/Binaries/WinGDK"
    for ancestor in game_dir.ancestors().skip(1).take(5) {
        let p_backup = ancestor.join("_DLSS5_Backup");
        if p_backup.exists() {
            return p_backup;
        }
    }

    // 3. Canonicalized path (resolving NTFS Junctions / symlinks like Xbox WindowsApps -> Games\...\Content)
    if let Ok(canon) = fs::canonicalize(game_dir) {
        let norm = strip_verbatim(&canon);
        let c_direct = norm.join("_DLSS5_Backup");
        if c_direct.exists() {
            return c_direct;
        }
        for ancestor in norm.ancestors().skip(1).take(5) {
            let p_backup = ancestor.join("_DLSS5_Backup");
            if p_backup.exists() {
                return p_backup;
            }
        }
    }

    direct
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

pub fn read_added_history(game_dir: &Path) -> Option<AddedHistory> {
    let bdir = backup_dir(game_dir);
    let hist_path = bdir.join("added_history.json");
    if hist_path.exists() {
        if let Ok(bytes) = fs::read(&hist_path) {
            if let Ok(h) = serde_json::from_slice::<AddedHistory>(&bytes) {
                return Some(h);
            }
        }
    }
    None
}

pub fn read_or_backfill_added_history(game_dir: &Path) -> AddedHistory {
    if let Some(h) = read_added_history(game_dir) {
        return h;
    }

    let bdir = backup_dir(game_dir);
    let mut history = AddedHistory::default();

    if bdir.exists() {
        // 1. Active manifest
        if let Some(m) = read_manifest(game_dir) {
            for f in m.added {
                history.files.insert(f);
            }
            for d in m.added_dirs {
                history.dirs.insert(d);
            }
        }

        // 2. Pending switch manifest
        let pending_path = bdir.join("pending-switch.json");
        if let Ok(bytes) = fs::read(&pending_path) {
            if let Ok(pending) = serde_json::from_slice::<ActiveManifest>(&bytes) {
                for f in pending.added {
                    history.files.insert(f);
                }
                for d in pending.added_dirs {
                    history.dirs.insert(d);
                }
            }
        }

        // 3. Historical done manifests
        if let Ok(entries) = fs::read_dir(&bdir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.starts_with("manifest.json.done-") {
                    if let Ok(bytes) = fs::read(entry.path()) {
                        if let Ok(past_m) = serde_json::from_slice::<ActiveManifest>(&bytes) {
                            for f in past_m.added {
                                history.files.insert(f);
                            }
                            for d in past_m.added_dirs {
                                history.dirs.insert(d);
                            }
                        }
                    }
                }
            }
        }

        if !history.files.is_empty() || !history.dirs.is_empty() {
            let hist_path = bdir.join("added_history.json");
            if let Ok(bytes) = serde_json::to_vec_pretty(&history) {
                let _ = fs::write(hist_path, bytes);
                crate::core::logger::info("journal", &format!("Backfilled cumulative added history for {}: {} files, {} dirs", game_dir.display(), history.files.len(), history.dirs.len()));
            }
        }
    }

    history
}

pub fn record_added_history(game_dir: &Path, added_files: &[String], added_dirs: &[String]) -> std::io::Result<()> {
    if added_files.is_empty() && added_dirs.is_empty() {
        return Ok(());
    }

    let bdir = backup_dir(game_dir);
    fs::create_dir_all(&bdir)?;

    let mut history = read_or_backfill_added_history(game_dir);
    let mut modified = false;

    for f in added_files {
        if history.files.insert(f.clone()) {
            modified = true;
        }
    }
    for d in added_dirs {
        if history.dirs.insert(d.clone()) {
            modified = true;
        }
    }

    if modified || !bdir.join("added_history.json").exists() {
        let bytes = serde_json::to_vec_pretty(&history)?;
        fs::write(bdir.join("added_history.json"), bytes)?;
        crate::core::logger::debug("journal", &format!("Updated cumulative added history for {}: {} files, {} dirs", game_dir.display(), history.files.len(), history.dirs.len()));
    }

    Ok(())
}

pub fn save_manifest(game_dir: &Path, manifest: &ActiveManifest) -> std::io::Result<()> {
    crate::core::logger::info("journal", &format!("Saving active manifest for {}: route={}, replaced={}, added={}", game_dir.display(), manifest.route, manifest.replaced.len(), manifest.added.len()));
    let bdir = backup_dir(game_dir);
    fs::create_dir_all(&bdir)?;
    let _ = record_added_history(game_dir, &manifest.added, &manifest.added_dirs);
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

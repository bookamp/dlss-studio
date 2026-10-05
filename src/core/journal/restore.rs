use std::fs;
use std::path::Path;
use super::clean::{archive_added_history, clean_untracked_mods, clean_untracked_mods_with_exe, is_corrupted_or_mod_backup};
use super::manifest::{
    backup_dir, prune_old_manifests, read_manifest, read_or_backfill_added_history,
    resolve_target_path, ActiveManifest,
};

pub fn has_backup_available(game_dir: &Path) -> bool {
    let bdir = backup_dir(game_dir);
    bdir.join("manifest.json").exists() || bdir.join("pending-switch.json").exists()
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

    // Purge any file recorded in cumulative added history
    let history = read_or_backfill_added_history(game_dir);
    for rel in &history.files {
        let target_file = resolve_target_path(game_dir, rel);
        if target_file.exists() {
            let _ = fs::remove_file(&target_file);
            crate::core::logger::debug("restore", &format!("Removed historically added file from cumulative history: {}", target_file.display()));
        }
    }
    for rel in history.dirs.iter().rev() {
        let target_dir = resolve_target_path(game_dir, rel);
        if target_dir.exists() {
            let _ = fs::remove_dir_all(&target_dir);
            crate::core::logger::debug("restore", &format!("Removed historically added directory from cumulative history: {}", target_dir.display()));
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
    let _ = archive_added_history(game_dir);

    crate::core::logger::info("restore", &format!("Restore successfully completed for {}", game_dir.display()));

    Ok(true)
}

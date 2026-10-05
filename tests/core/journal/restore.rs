use dlss_studio::core::journal::restore::*;
use dlss_studio::core::journal::manifest::{backup_dir, ActiveManifest, ManifestItem};
use crate::common::TempDir;
use std::fs;

#[test]
fn test_has_backup_available() {
    let temp = TempDir::new("restore_has_backup");
    let bdir = temp.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    assert!(!has_backup_available(temp.path()));

    let manifest = ActiveManifest {
        route: "optiscaler".to_string(),
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(bdir.join("manifest.json"), &bytes).unwrap();
    assert!(has_backup_available(temp.path()));
}

#[test]
fn test_running_game_guard_prevents_restore() {
    let procs = dlss_studio::core::install_guards::get_running_processes();
    if procs.is_empty() {
        return;
    }
    let running_proc_name = &procs[0].name;

    let temp = TempDir::new("guard_restore");
    let running_dummy_exe = temp.join(running_proc_name);
    fs::write(&running_dummy_exe, b"MZ").unwrap();

    let manifest = ActiveManifest {
        game_exe: Some(running_proc_name.clone()),
        ..Default::default()
    };
    let bdir = backup_dir(temp.path());
    fs::create_dir_all(&bdir).unwrap();
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();

    let restore_res = restore_game(temp.path());
    assert!(restore_res.is_err(), "Restore must be blocked when game process is active");
    let rest_err = restore_res.unwrap_err().to_string();
    assert!(rest_err.contains("Close the game"), "Error must tell user to close the game first: {}", rest_err);
}

#[test]
fn test_restore_game_purges_corrupted_backup_and_historically_added_files() {
    let temp = TempDir::new("restore_clean");

    // Vanilla game file
    fs::write(temp.join("sl.interposer.dll"), b"VANILLA_INTERPOSER").unwrap();
    // Leftover injected file sitting in game dir
    fs::write(temp.join("sl.pcl.dll"), b"INJECTED_PCL").unwrap();
    // OptiScaler files
    fs::write(temp.join("dxgi.dll"), b"OPTISCALER_DXGI").unwrap();

    let bdir = temp.join("_DLSS5_Backup");
    let prefix = "originals/test1";
    let orig_dir = bdir.join(prefix);
    fs::create_dir_all(&orig_dir).unwrap();

    // Legitimate vanilla backup
    fs::write(orig_dir.join("sl.interposer.dll"), b"VANILLA_INTERPOSER").unwrap();
    // CORRUPTED BACKUP: sl.pcl.dll was mistakenly put in backup by prior bug
    fs::write(orig_dir.join("sl.pcl.dll"), b"INJECTED_PCL").unwrap();

    // Done manifest from an even earlier run that had added sl.pcl.dll
    let done_manifest = ActiveManifest {
        added: vec!["sl.pcl.dll".to_string()],
        ..Default::default()
    };
    fs::write(bdir.join("manifest.json.done-100"), serde_json::to_vec(&done_manifest).unwrap()).unwrap();

    // Current active manifest mistakenly has sl.pcl.dll in replaced
    let active_manifest = ActiveManifest {
        backup_prefix: Some(prefix.to_string()),
        replaced: vec![
            ManifestItem { rel: "sl.interposer.dll".to_string(), old_hash: None, kind: None },
            ManifestItem { rel: "sl.pcl.dll".to_string(), old_hash: None, kind: None },
        ],
        added: vec!["dxgi.dll".to_string()],
        ..Default::default()
    };
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&active_manifest).unwrap()).unwrap();

    // Execute restore
    let res = restore_game(temp.path()).expect("restore must succeed");
    assert!(res);

    // Assert pristine state:
    // 1. sl.interposer.dll was restored
    assert_eq!(fs::read(temp.join("sl.interposer.dll")).unwrap(), b"VANILLA_INTERPOSER");
    // 2. dxgi.dll was deleted
    assert!(!temp.join("dxgi.dll").exists());
    // 3. sl.pcl.dll was PURGED (not restored from corrupted backup)
    assert!(!temp.join("sl.pcl.dll").exists(), "Corrupted backup sl.pcl.dll must be purged, not restored!");
}

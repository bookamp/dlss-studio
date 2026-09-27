use dlss_studio::core::journal::*;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_has_backup_available_and_read_done_manifest() {
    let temp = TempDir::new("journal");
    let bdir = temp.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    assert!(!has_backup_available(temp.path()));
    assert!(read_manifest(temp.path()).is_none());

    // Create an archived manifest
    let manifest = ActiveManifest {
        route: "optiscaler".to_string(),
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(bdir.join("manifest.json.done-123456789"), &bytes).unwrap();

    // Archived manifest represents a completed restore; should NOT trigger active backup or active manifest
    assert!(!has_backup_available(temp.path()), "Archived manifest must not trigger has_backup_available");
    assert!(read_manifest(temp.path()).is_none(), "read_manifest must not fall back to archived manifest");

    // Can still be inspected via read_latest_done_manifest
    let read = read_latest_done_manifest(temp.path());
    assert!(read.is_some(), "read_latest_done_manifest should find archived manifest");
    assert_eq!(read.unwrap().route, "optiscaler");

    // When active manifest.json is present, both return true/Some
    fs::write(bdir.join("manifest.json"), &bytes).unwrap();
    assert!(has_backup_available(temp.path()));
    assert!(read_manifest(temp.path()).is_some());
}

#[test]
fn test_clean_untracked_mods() {
    let temp = TempDir::new("clean");

    let opti_ini = temp.join("OptiScaler.ini");
    let reshade_dll = temp.join("ReShade64.dll");
    let addon = temp.join("renodx-mfgunlock.addon64");
    let safe_file = temp.join("Game.exe");
    let proxy_dxgi = temp.join("dxgi.dll");

    fs::write(&opti_ini, b"ini").unwrap();
    fs::write(&reshade_dll, b"dll").unwrap();
    fs::write(&addon, b"addon").unwrap();
    fs::write(&safe_file, b"exe").unwrap();

    let mut proxy_bytes = vec![0u8; 100_000];
    proxy_bytes[50_000..50_010].copy_from_slice(b"OptiScaler");
    fs::write(&proxy_dxgi, proxy_bytes).unwrap();

    let removed = clean_untracked_mods(temp.path()).unwrap();
    assert_eq!(removed.len(), 4);
    assert!(!opti_ini.exists());
    assert!(!reshade_dll.exists());
    assert!(!addon.exists());
    assert!(!proxy_dxgi.exists(), "Proxy dxgi.dll must be removed");
    assert!(safe_file.exists(), "Original game executable must not be deleted");
}

#[test]
fn test_clean_untracked_mods_nested_reshade() {
    let temp = TempDir::new("clean_nested");
    let nested_dir = temp.join("bin").join("x64");
    fs::create_dir_all(&nested_dir).unwrap();

    let nested_exe = nested_dir.join("MockCyberpunk2077.exe");
    let reshade_dxgi = nested_dir.join("dxgi.dll");
    let renodx_addon = nested_dir.join("renodx-dlss5.addon64");
    let mfg_addon = nested_dir.join("renodx-mfgunlock.addon64");
    let reshade_ini = nested_dir.join("ReShade.ini");

    fs::write(&nested_exe, b"MZ dummy exe").unwrap();
    fs::write(&renodx_addon, b"addon").unwrap();
    fs::write(&mfg_addon, b"mfg").unwrap();
    fs::write(&reshade_ini, b"[INPUT]\nKeyOverlay=36").unwrap();

    let mut reshade_bytes = vec![0u8; 100_000];
    reshade_bytes[50_000..50_007].copy_from_slice(b"ReShade");
    fs::write(&reshade_dxgi, reshade_bytes).unwrap();

    let removed = clean_untracked_mods_with_exe(temp.path(), Some(&nested_exe)).unwrap();
    assert!(removed.contains(&"dxgi.dll".to_string()));
    assert!(removed.contains(&"renodx-dlss5.addon64".to_string()));
    assert!(removed.contains(&"renodx-mfgunlock.addon64".to_string()));
    assert!(removed.contains(&"ReShade.ini".to_string()));

    assert!(!reshade_dxgi.exists(), "Nested dxgi.dll must be cleaned");
    assert!(!renodx_addon.exists(), "Nested renodx-dlss5.addon64 must be cleaned");
    assert!(!mfg_addon.exists(), "Nested renodx-mfgunlock.addon64 must be cleaned");
    assert!(!reshade_ini.exists(), "Nested ReShade.ini must be cleaned");
    assert!(nested_exe.exists(), "Game executable must remain intact");
}

#[test]
fn test_running_game_guard_prevents_clean_and_restore() {
    let procs = dlss_studio::core::install_guards::get_running_processes();
    if procs.is_empty() {
        return;
    }
    let running_proc_name = &procs[0].name;

    let temp = TempDir::new("guard");
    let running_dummy_exe = temp.join(running_proc_name);
    fs::write(&running_dummy_exe, b"MZ").unwrap();

    // 1. clean_untracked_mods_with_exe must reject cleaning while target process is running
    let clean_res = clean_untracked_mods_with_exe(temp.path(), Some(&running_dummy_exe));
    assert!(clean_res.is_err(), "Cleaning must be blocked when game process is active");
    let err_str = clean_res.unwrap_err().to_string();
    assert!(err_str.contains("Close the game"), "Error must tell user to close the game first: {}", err_str);

    // 2. restore_game must also reject restoring while target process is running
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
fn test_history_and_manifest_serialization() {
    let temp = TempDir::new("journal_ser");

    // Manifest path
    let m_path = backup_dir(temp.path()).join("manifest.json");
    assert!(m_path.ends_with("manifest.json"));

    // Read manifest on non-existent file
    assert!(read_manifest(temp.path()).is_none());

    // Read manifest with invalid JSON
    fs::create_dir_all(backup_dir(temp.path())).unwrap();
    fs::write(&m_path, b"invalid-json").unwrap();
    assert!(read_manifest(temp.path()).is_none());
}

#[test]
fn test_history_json_dual_format_and_version_migration() {
    let current_v = "2.0.0";

    // 1. Legacy naked JSON array
    let legacy_json = br#"[
        {
            "date": "2026-09-12 18:30:00",
            "dir": "C:\\Games\\Cyberpunk 2077",
            "game_name": "Cyberpunk 2077",
            "action": "Feeder",
            "replaced": 2,
            "added": 4
        }
    ]"#;

    let (rows, migration) = parse_history_bytes(legacy_json, current_v);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].game_name.as_deref(), Some("Cyberpunk 2077"));
    assert!(migration.is_some(), "Legacy history must trigger migration");
    let migrated = migration.unwrap();
    assert_eq!(migrated.version, current_v);
    assert_eq!(migrated.entries.len(), 1);

    // 2. Versioned format matching current version
    let versioned_json = serde_json::to_vec_pretty(&migrated).unwrap();
    let (rows2, migration2) = parse_history_bytes(&versioned_json, current_v);
    assert_eq!(rows2.len(), 1);
    assert!(migration2.is_none(), "Matching version must not trigger re-migration");

    // 3. Older versioned format (e.g. 1.0.0) -> upgrades to 2.0.0
    let older_file = HistoryFile {
        version: "1.0.0".to_string(),
        entries: rows2,
    };
    let older_json = serde_json::to_vec_pretty(&older_file).unwrap();
    let (rows3, migration3) = parse_history_bytes(&older_json, current_v);
    assert_eq!(rows3.len(), 1);
    assert!(migration3.is_some(), "Older version must trigger upgrade");
    assert_eq!(migration3.unwrap().version, current_v);
}

#[test]
fn test_prune_old_manifests_keeps_last_5_and_cleans_orphaned_originals() {
    let temp = TempDir::new("prune");
    let bdir = temp.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    // Create 8 archived done manifests (ts 100 to 800)
    for i in 1..=8 {
        let ts = 1000 + i * 100;
        let prefix = format!("originals/{}", ts);
        let orig_dir = bdir.join(&prefix);
        fs::create_dir_all(&orig_dir).unwrap();
        fs::write(orig_dir.join("dxgi.dll"), b"mock-orig").unwrap();

        let manifest = ActiveManifest {
            backup_prefix: Some(prefix),
            ..Default::default()
        };
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        fs::write(bdir.join(format!("manifest.json.done-{}", ts)), bytes).unwrap();
    }

    // Active manifest references originals/1800
    let active_manifest = ActiveManifest {
        backup_prefix: Some("originals/1800".to_string()),
        ..Default::default()
    };
    let active_bytes = serde_json::to_vec_pretty(&active_manifest).unwrap();
    fs::write(bdir.join("manifest.json"), active_bytes).unwrap();

    // Run pruner keeping top 5
    let pruned = prune_old_manifests(temp.path(), 5);
    assert_eq!(pruned, 3, "Must prune 3 older manifests out of 8");

    // Verify remaining done manifests: 1800, 1700, 1600, 1500, 1400 should exist
    assert!(bdir.join("manifest.json.done-1800").exists());
    assert!(bdir.join("manifest.json.done-1700").exists());
    assert!(bdir.join("manifest.json.done-1600").exists());
    assert!(bdir.join("manifest.json.done-1500").exists());
    assert!(bdir.join("manifest.json.done-1400").exists());

    // 1300, 1200, 1100 must be deleted
    assert!(!bdir.join("manifest.json.done-1300").exists());
    assert!(!bdir.join("manifest.json.done-1200").exists());
    assert!(!bdir.join("manifest.json.done-1100").exists());

    // Verify orphaned originals/1300, 1200, 1100 were cleaned up
    assert!(!bdir.join("originals/1300").exists());
    assert!(!bdir.join("originals/1200").exists());
    assert!(!bdir.join("originals/1100").exists());

    // Retained originals/1400 to 1800 must still exist
    assert!(bdir.join("originals/1400").exists());
    assert!(bdir.join("originals/1800").exists());
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

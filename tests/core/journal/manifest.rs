use dlss_studio::core::journal::manifest::*;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_has_backup_available_and_read_done_manifest() {
    let temp = TempDir::new("journal_manifest");
    let bdir = temp.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    assert!(read_manifest(temp.path()).is_none());

    // Create an archived manifest
    let manifest = ActiveManifest {
        route: "optiscaler".to_string(),
        ..Default::default()
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(bdir.join("manifest.json.done-123456789"), &bytes).unwrap();

    // Archived manifest represents a completed restore; should NOT trigger active manifest
    assert!(read_manifest(temp.path()).is_none(), "read_manifest must not fall back to archived manifest");

    // Can still be inspected via read_latest_done_manifest
    let read = read_latest_done_manifest(temp.path());
    assert!(read.is_some(), "read_latest_done_manifest should find archived manifest");
    assert_eq!(read.unwrap().route, "optiscaler");

    // When active manifest.json is present, return Some
    fs::write(bdir.join("manifest.json"), &bytes).unwrap();
    assert!(read_manifest(temp.path()).is_some());
}

#[test]
fn test_manifest_serialization() {
    let temp = TempDir::new("manifest_ser");

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

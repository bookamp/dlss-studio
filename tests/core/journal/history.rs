use dlss_studio::core::journal::history::*;

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
fn test_now_timestamp_str() {
    let ts = now_timestamp_str();
    assert!(!ts.is_empty());
    assert!(ts.contains("UTC") || ts == "Recently");
}

#[test]
fn test_save_and_read_history_lifecycle() {
    let file = HistoryFile {
        version: "2.0.2".to_string(),
        entries: vec![
            HistoryRow {
                date: "2026-10-01 12:00:00 UTC".to_string(),
                dir: "C:\\Games\\SampleGame".to_string(),
                game_name: Some("Sample Game".to_string()),
                action: "Native".to_string(),
                replaced: 1,
                added: 2,
            },
        ],
    };

    let save_res = save_history_file(&file);
    assert!(save_res.is_ok());

    let loaded = read_history();
    assert!(!loaded.is_empty());
    assert!(loaded.iter().any(|r| r.game_name.as_deref() == Some("Sample Game")));

    // Test append_history early return on test prefix
    let test_row = HistoryRow {
        date: "now".to_string(),
        dir: "C:\\dlss_test_123\\game".to_string(),
        game_name: None,
        action: "Clean".to_string(),
        replaced: 0,
        added: 0,
    };
    assert!(append_history(&test_row).is_ok());
}

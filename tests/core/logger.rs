use dlss_studio::core::logger::{
    get_log_file_path, info, log_to_file, parse_date_to_epoch_approx, prune_log_file,
    RETENTION_SECS,
};
use crate::common::TempDir;
use std::fs;
use std::io::{BufRead, BufReader};

#[test]
fn test_date_parsing() {
    let epoch = parse_date_to_epoch_approx("2026-09-10").unwrap();
    assert!(epoch > 1_700_000_000);
}

#[test]
fn test_log_and_prune() {
    let temp_dir = TempDir::new("logger");
    let log_file = temp_dir.path().join("dlss-studio.log");

    // Write an old entry (from 2020) and a recent entry
    let old_entry = "[2020-01-01 12:00:00.000] [INFO] [test] Old message\n";
    let new_entry = "[2026-09-10 12:00:00.000] [INFO] [test] New message\n";
    fs::write(&log_file, format!("{}{}", old_entry, new_entry)).unwrap();

    // Run prune logic directly on this file
    let file = fs::File::open(&log_file).unwrap();
    let reader = BufReader::new(file);
    let now_s = parse_date_to_epoch_approx("2026-09-10").unwrap();
    let cutoff_s = now_s - RETENTION_SECS;
    let mut kept = Vec::new();
    for line in reader.lines() {
        let l = line.unwrap();
        let date_part = &l[1..11];
        if let Some(line_epoch) = parse_date_to_epoch_approx(date_part) {
            if line_epoch + 86_400 < cutoff_s {
                continue;
            }
        }
        kept.push(l);
    }

    assert_eq!(kept.len(), 1);
    assert!(kept[0].contains("New message"));
}

#[test]
fn test_live_logger_write() {
    info("test", "Testing single rolling log file write");
    let path = get_log_file_path();
    assert!(path.exists(), "Log file should exist at {}", path.display());
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("Testing single rolling log file write"));
}

#[test]
fn test_prune_log_file_custom_path() {
    let temp_dir = TempDir::new("prune_custom");
    let custom_log = temp_dir.path().join("big-picture.log");

    let old_entry = "[2020-01-01 12:00:00.000] [INFO] [bp::ui] Old stale entry\n";
    let new_entry = "[2026-09-20 12:00:00.000] [INFO] [bp::ui] Fresh entry\n";
    fs::write(&custom_log, format!("{}{}", old_entry, new_entry)).unwrap();

    prune_log_file(&custom_log);

    let content = fs::read_to_string(&custom_log).unwrap();
    assert!(!content.contains("Old stale entry"), "Stale entry older than 7 days must be pruned");
    assert!(content.contains("Fresh entry"), "Recent entry must be preserved");
}

#[test]
fn test_log_to_file_custom_path() {
    let temp_dir = TempDir::new("custom_log");
    let custom_log = temp_dir.path().join("custom.log");

    log_to_file(&custom_log, "INFO", "test_target", "Hello from custom logger");
    assert!(custom_log.exists());
    let content = fs::read_to_string(&custom_log).unwrap();
    assert!(content.contains("[INFO]"));
    assert!(content.contains("[test_target]"));
    assert!(content.contains("Hello from custom logger"));
}

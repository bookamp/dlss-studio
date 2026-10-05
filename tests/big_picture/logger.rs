use dlss_studio::big_picture::logger::*;
use std::fs;

#[test]
fn test_bp_log_path_ends_with_big_picture_log() {
    let path = get_bp_log_path();
    assert_eq!(
        path.file_name().and_then(|f| f.to_str()),
        Some("big-picture.log"),
        "Path must strictly point to big-picture.log"
    );
}

#[test]
fn test_bp_logger_writes_and_appends() {
    let unique_msg = format!("Test Big Picture log message {}", std::time::SystemTime::now().elapsed().unwrap_or_default().as_nanos());
    info("test_suite", &unique_msg);

    let path = get_bp_log_path();
    assert!(path.exists(), "big-picture.log must exist after writing");

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("[INFO]"));
    assert!(content.contains("[bp::test_suite]"));
    assert!(content.contains(&unique_msg));
}

#[test]
fn test_bp_logger_levels() {
    let tag = format!("levels_test_{}", std::time::SystemTime::now().elapsed().unwrap_or_default().as_nanos());
    warn(&tag, "Warn message");
    error(&tag, "Error message");
    debug(&tag, "Debug message");

    let path = get_bp_log_path();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains(&format!("[WARN] [bp::{}] Warn message", tag)));
    assert!(content.contains(&format!("[ERROR] [bp::{}] Error message", tag)));
    assert!(content.contains(&format!("[DEBUG] [bp::{}] Debug message", tag)));
}

#[test]
fn test_bp_logger_rolling_prune() {
    let temp_dir = std::env::temp_dir().join(format!("dlss_bp_logger_test_{}", std::time::SystemTime::now().elapsed().unwrap_or_default().as_nanos()));
    let _ = fs::create_dir_all(&temp_dir);
    let test_log = temp_dir.join("big-picture.log");

    // Write an old entry (from 2021) and a recent entry
    let old_entry = "[2021-01-01 00:00:00.000] [INFO] [bp::supervisor] Stale session log\n";
    fs::write(&test_log, old_entry).unwrap();
    dlss_studio::core::logger::log_to_file(&test_log, "INFO", "bp::supervisor", "Active session log");

    // Prune test file
    dlss_studio::core::logger::prune_log_file(&test_log);

    let content = fs::read_to_string(&test_log).unwrap();
    assert!(!content.contains("Stale session log"), "Old log entries > 7 days must be pruned");
    assert!(content.contains("Active session log"), "Recent log entries must be preserved");

    let _ = fs::remove_dir_all(&temp_dir);
}

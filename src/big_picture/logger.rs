//! Dedicated rolling 7-day logger for DLSS Studio Big Picture Mode.
//!
//! Strictly isolated downstream in `src/big_picture/` per AGENTS.md Rule 5.
//! Reuses core logging pruner from `crate::core::logger` for single source of truth.

use std::path::PathBuf;

/// Resolves the absolute path to `big-picture.log` in the application data directory.
pub fn get_bp_log_path() -> PathBuf {
    crate::core::state::get_appdata_dir().join("big-picture.log")
}

/// Logs a formatted message to `big-picture.log` with rolling 7-day pruning.
pub fn log(level: &str, target: &str, message: &str) {
    let path = get_bp_log_path();
    let bp_target = if target.starts_with("bp::") {
        target.to_string()
    } else {
        format!("bp::{}", target)
    };
    crate::core::logger::log_to_file(&path, level, &bp_target, message);
}

pub fn info(target: &str, message: &str) {
    log("INFO", target, message);
}

#[allow(dead_code)]
pub fn warn(target: &str, message: &str) {
    log("WARN", target, message);
}

#[allow(dead_code)]
pub fn error(target: &str, message: &str) {
    log("ERROR", target, message);
}

#[allow(dead_code)]
pub fn debug(target: &str, message: &str) {
    log("DEBUG", target, message);
}

/// Prunes entries older than 7 days from `big-picture.log`.
#[allow(dead_code)]
pub fn prune_old_entries() {
    crate::core::logger::prune_log_file(&get_bp_log_path());
}

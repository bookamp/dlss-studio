//! In-memory ephemeral session logger and multilingual time elapsed formatter.

use std::sync::Mutex;

static SESSION_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[doc(hidden)]
pub static STATE_TEST_MUTEX: Mutex<()> = Mutex::new(());

pub fn log_message(msg: &str) {
    let disk_msg = crate::core::i18n::format_log_entry("en", msg);
    crate::core::logger::info("ui", &disk_msg);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sec_in_day = now % 86400;
    let hours = sec_in_day / 3600;
    let minutes = (sec_in_day % 3600) / 60;
    let seconds = sec_in_day % 60;
    let line = format!("[{:02}:{:02}:{:02}] {}", hours, minutes, seconds, msg);
    if let Ok(mut lock) = SESSION_LOG.lock() {
        lock.push(line);
        if lock.len() > 120 {
            lock.remove(0);
        }
    }
}

pub fn get_session_log() -> Vec<String> {
    SESSION_LOG.lock().map(|l| l.clone()).unwrap_or_default()
}

pub fn ago_localized(lang: &str, ts_ms: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let diff_s = if now_ms > ts_ms { (now_ms - ts_ms) / 1000 } else { 0 };
    if diff_s < 60 {
        crate::core::i18n::t(lang, "time_just_now").to_string()
    } else if diff_s < 3600 {
        crate::core::i18n::t_param(lang, "time_minutes_ago", &(diff_s / 60).to_string())
    } else if diff_s < 86400 {
        crate::core::i18n::t_param(lang, "time_hours_ago", &(diff_s / 3600).to_string())
    } else if diff_s < 172800 {
        crate::core::i18n::t(lang, "time_yesterday").to_string()
    } else {
        crate::core::i18n::t_param(lang, "time_days_ago", &(diff_s / 86400).to_string())
    }
}

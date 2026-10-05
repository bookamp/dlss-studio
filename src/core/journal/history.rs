use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HistoryRow {
    pub date: String,
    pub dir: String,
    #[serde(default)]
    pub game_name: Option<String>,
    pub action: String,
    pub replaced: usize,
    pub added: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryFile {
    pub version: String,
    pub entries: Vec<HistoryRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum HistoryStorageFormat {
    Versioned(HistoryFile),
    Legacy(Vec<HistoryRow>),
}

pub fn history_path() -> PathBuf {
    crate::core::state::get_appdata_dir().join("history.json")
}

pub fn save_history_file(file: &HistoryFile) -> std::io::Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(file)?;
    fs::write(path, bytes)?;
    Ok(())
}

pub fn parse_history_bytes(bytes: &[u8], current_version: &str) -> (Vec<HistoryRow>, Option<HistoryFile>) {
    if let Ok(format) = serde_json::from_slice::<HistoryStorageFormat>(bytes) {
        match format {
            HistoryStorageFormat::Versioned(file) => {
                let filtered: Vec<HistoryRow> = file.entries.into_iter()
                    .filter(|r| !r.dir.contains("dlss_test_") && !r.dir.contains("dlss_addon_test_"))
                    .collect();
                if file.version != current_version {
                    crate::core::logger::info("journal", &format!("Migrating history.json from {} to {}", file.version, current_version));
                    let upgraded = HistoryFile {
                        version: current_version.to_string(),
                        entries: filtered.clone(),
                    };
                    return (filtered, Some(upgraded));
                }
                return (filtered, None);
            }
            HistoryStorageFormat::Legacy(rows) => {
                let filtered: Vec<HistoryRow> = rows.into_iter()
                    .filter(|r| !r.dir.contains("dlss_test_") && !r.dir.contains("dlss_addon_test_"))
                    .collect();
                crate::core::logger::info("journal", &format!("Migrating legacy history.json to version {}", current_version));
                let upgraded = HistoryFile {
                    version: current_version.to_string(),
                    entries: filtered.clone(),
                };
                return (filtered, Some(upgraded));
            }
        }
    }
    (Vec::new(), None)
}

pub fn read_history() -> Vec<HistoryRow> {
    let path = history_path();
    if let Ok(bytes) = fs::read(&path) {
        let (rows, migration) = parse_history_bytes(&bytes, env!("CARGO_PKG_VERSION"));
        if let Some(upgraded) = migration {
            let _ = save_history_file(&upgraded);
        }
        return rows;
    }
    Vec::new()
}

pub fn append_history(row: &HistoryRow) -> std::io::Result<()> {
    if row.dir.contains("dlss_test_") || row.dir.contains("dlss_addon_test_") || row.dir.contains("test_") {
        return Ok(());
    }
    let mut history = read_history();
    history.push(row.clone());
    let file = HistoryFile {
        version: env!("CARGO_PKG_VERSION").to_string(),
        entries: history,
    };
    save_history_file(&file)?;
    Ok(())
}

pub fn now_timestamp_str() -> String {
    let now = std::time::SystemTime::now();
    if let Ok(dur) = now.duration_since(std::time::UNIX_EPOCH) {
        let secs = dur.as_secs();
        let days = secs / 86400;
        let day_secs = secs % 86400;
        let hours = day_secs / 3600;
        let mins = (day_secs % 3600) / 60;
        let mut y = 1970;
        let mut d = days;
        loop {
            let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
            let ydays = if leap { 366 } else { 365 };
            if d < ydays { break; }
            d -= ydays;
            y += 1;
        }
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut m = 1;
        for (idx, &dim) in mdays.iter().enumerate() {
            if d < dim { m = idx + 1; break; }
            d -= dim;
            y += 1;
        }
        format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", y, m, d + 1, hours, mins)
    } else {
        "Recently".to_string()
    }
}

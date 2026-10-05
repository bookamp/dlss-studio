//! Filesystem path normalization, protection checks, and AppData directory resolution.

use std::fs;
use std::path::{Path, PathBuf};

use super::model::StorageConfig;

pub fn normalize_path_str(p: &str) -> String {
    p.trim().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

pub fn normalize_game_path(p: &Path) -> String {
    normalize_path_str(&p.to_string_lossy())
}

pub fn clean_path_separators(p: &Path) -> PathBuf {
    let s = p.to_string_lossy().replace('/', "\\");
    let trimmed = s.trim_end_matches('\\');
    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        let mut chars = trimmed.chars();
        let drive = chars.next().unwrap().to_ascii_uppercase();
        let rest: String = chars.collect();
        PathBuf::from(format!("{}{}", drive, rest))
    } else {
        PathBuf::from(trimmed)
    }
}

pub fn is_path_protected<P: AsRef<Path>>(path: P) -> bool {
    let path_str = path.as_ref().to_string_lossy().to_lowercase().replace('/', "\\");
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let pf_lower = pf.to_lowercase().replace('/', "\\");
        if path_str.starts_with(&pf_lower) {
            return true;
        }
    } else if path_str.starts_with(r"c:\program files") {
        return true;
    }

    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        let pf86_lower = pf86.to_lowercase().replace('/', "\\");
        if path_str.starts_with(&pf86_lower) {
            return true;
        }
    } else if path_str.starts_with(r"c:\program files (x86)") {
        return true;
    }

    if let Ok(windir) = std::env::var("SystemRoot") {
        let win_lower = windir.to_lowercase().replace('/', "\\");
        if path_str.starts_with(&win_lower) {
            return true;
        }
    } else if path_str.starts_with(r"c:\windows") {
        return true;
    }

    if path_str.contains(r"\windowsapps") {
        return true;
    }

    false
}

pub fn resolve_appdata_dir_internal(
    exe_name: &str,
    exe_dir: Option<&Path>,
    storage_json_content: Option<&str>,
    env_programdata: Option<&str>,
    env_appdata: Option<&str>,
) -> PathBuf {
    // 1. Portable build / executable check
    let lower_name = exe_name.to_lowercase();
    if lower_name.contains("portable") {
        if let Some(appdata) = env_appdata {
            return PathBuf::from(appdata).join("dlss-5-studio");
        } else {
            return PathBuf::from(".").join(".appdata");
        }
    }

    // 2. storage.json in executable directory
    if let Some(content) = storage_json_content {
        if let Ok(cfg) = serde_json::from_str::<StorageConfig>(content) {
            let p = PathBuf::from(cfg.data_dir.trim());
            if !p.as_os_str().is_empty() {
                return p;
            }
        }
    }

    // 3. If installed in an unprotected location (e.g. D:\Games\DLSS 5 Studio or C:\Games\...), default to <exe_dir>\Data
    if let Some(dir) = exe_dir {
        if !is_path_protected(dir) {
            return dir.join("data");
        }
    }

    // 4. If installed in a protected location (Program Files / Windows), default to ProgramData
    if let Some(pd) = env_programdata {
        return PathBuf::from(pd).join("dlss-5-studio");
    }

    // 5. Ultimate fallback
    if let Some(appdata) = env_appdata {
        PathBuf::from(appdata).join("dlss-5-studio")
    } else {
        PathBuf::from(".").join(".appdata")
    }
}

pub fn get_appdata_dir() -> PathBuf {
    let curr_exe = std::env::current_exe().ok();
    let exe_name = curr_exe
        .as_ref()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_default();
    let exe_dir = curr_exe.as_ref().and_then(|p| p.parent());

    let storage_json = exe_dir.and_then(|d| fs::read_to_string(d.join("storage.json")).ok());
    let prog_data = std::env::var("ProgramData").ok();
    let app_data = std::env::var("APPDATA").ok();

    let resolved = resolve_appdata_dir_internal(
        &exe_name,
        exe_dir,
        storage_json.as_deref(),
        prog_data.as_deref(),
        app_data.as_deref(),
    );

    let _ = fs::create_dir_all(&resolved);
    resolved
}

pub fn get_state_path() -> PathBuf {
    get_appdata_dir().join("library.json")
}

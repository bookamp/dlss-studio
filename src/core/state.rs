use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanCacheEntry {
    pub dir: String,
    pub ok: bool,
    pub installable: bool,
    pub api: Option<String>,
    pub bitness: Option<u32>,
    pub dx12: bool,
    pub exe: Option<String>,
    pub reason: Option<String>,
    pub dlss: Option<String>,
    #[serde(rename = "hasDlss")]
    pub has_dlss: bool,
    pub addon: bool,
    pub optiscaler: bool,
    pub reshade: Option<String>,
    #[serde(rename = "scannedAt")]
    pub scanned_at: u64,
    pub rules: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddonFileEntry {
    pub path: String,
    pub name: Option<String>,
    pub tag: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_addons() -> Vec<String> {
    vec![
        "builtin:renodx".to_string(),
        "builtin:mfgunlock".to_string(),
        "builtin:feeder".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentEntry {
    pub dir: String,
    pub at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomOverlayTheme {
    pub id: String,
    pub name: String,
    pub color: String,
}

pub const CURRENT_APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppState {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default, rename = "excludedRoots")]
    pub excluded_roots: Vec<String>,
    #[serde(default)]
    pub manual: Vec<String>,
    #[serde(default)]
    pub posters: HashMap<String, String>,
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub scans: HashMap<String, ScanCacheEntry>,
    #[serde(default)]
    pub recents: Vec<RecentEntry>,
    #[serde(default, rename = "apiOverrides")]
    pub api_overrides: HashMap<String, String>,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default = "default_true", rename = "groupGamesByStore")]
    pub group_games_by_store: bool,
    #[serde(default, rename = "autoScanDrives")]
    pub auto_scan_drives: bool,
    #[serde(default = "default_true", rename = "rustTheme")]
    pub rust_theme: bool,
    #[serde(default = "default_run_in_background", rename = "runInBackground")]
    pub run_in_background: bool,
    #[serde(default, rename = "cachedGames")]
    pub cached_games: Vec<crate::core::scan::GameEntry>,
    #[serde(default = "default_addons")]
    pub addons: Vec<String>,
    #[serde(default, rename = "addonFiles")]
    pub addon_files: Vec<AddonFileEntry>,
    #[serde(default = "default_false", rename = "overlayEnabled")]
    pub overlay_enabled: bool,
    #[serde(default = "default_overlay_theme", rename = "overlayTheme")]
    pub overlay_theme: String,
    #[serde(default = "default_overlay_hotkey", rename = "overlayHotkey")]
    pub overlay_hotkey: String,
    #[serde(default, rename = "customOverlayThemes")]
    pub custom_overlay_themes: Vec<CustomOverlayTheme>,
    #[serde(default, rename = "customNames")]
    pub custom_names: HashMap<String, String>,
    #[serde(default = "default_false", rename = "vibepolloAutoAdd")]
    pub vibepollo_auto_add: bool,
}

pub fn is_portable_executable() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        let name = exe.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        if name.contains("portable") {
            return true;
        }
    }
    false
}

pub fn default_run_in_background() -> bool {
    !is_portable_executable()
}

fn default_theme() -> String { "dark".to_string() }
fn default_lang() -> String { crate::core::i18n::detect_system_language() }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_overlay_theme() -> String { "green".to_string() }
fn default_overlay_hotkey() -> String { "F8".to_string() }

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: Some(CURRENT_APP_VERSION.to_string()),
            folders: Vec::new(),
            excluded_roots: Vec::new(),
            manual: Vec::new(),
            posters: HashMap::new(),
            hidden: Vec::new(),
            scans: HashMap::new(),
            recents: Vec::new(),
            api_overrides: HashMap::new(),
            theme: default_theme(),
            lang: default_lang(),
            group_games_by_store: true,
            auto_scan_drives: false,
            rust_theme: true,
            run_in_background: default_run_in_background(),
            cached_games: Vec::new(),
            addons: default_addons(),
            addon_files: Vec::new(),
            overlay_enabled: false,
            overlay_theme: default_overlay_theme(),
            overlay_hotkey: default_overlay_hotkey(),
            custom_overlay_themes: Vec::new(),
            custom_names: HashMap::new(),
            vibepollo_auto_add: false,
        }
    }
}

pub fn normalize_path_str(p: &str) -> String {
    p.trim().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

pub fn normalize_game_path(p: &std::path::Path) -> String {
    normalize_path_str(&p.to_string_lossy())
}

pub fn clean_path_separators(p: &std::path::Path) -> PathBuf {
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

impl AppState {
    pub fn get_custom_name(&self, dir: &std::path::Path) -> Option<&String> {
        let norm = normalize_game_path(dir);
        self.custom_names.get(&norm)
    }

    pub fn set_custom_name(&mut self, dir: &std::path::Path, name: &str) {
        let norm = normalize_game_path(dir);
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.custom_names.remove(&norm);
        } else {
            self.custom_names.insert(norm, trimmed.to_string());
        }
    }

    pub fn is_hidden(&self, dir: &std::path::Path) -> bool {
        let norm = normalize_game_path(dir);
        self.hidden.iter().any(|h| normalize_path_str(h) == norm)
    }

    pub fn hide_game(&mut self, dir: &std::path::Path) {
        let norm = normalize_game_path(dir);
        if !self.hidden.iter().any(|h| normalize_path_str(h) == norm) {
            self.hidden.push(dir.to_string_lossy().to_string());
        }
        self.manual.retain(|m| normalize_path_str(m) != norm);
        self.cached_games.retain(|g| normalize_game_path(&g.dir) != norm);
    }

    pub fn unhide_game(&mut self, dir: &std::path::Path) {
        let norm = normalize_game_path(dir);
        self.hidden.retain(|h| normalize_path_str(h) != norm);
    }

    pub fn unhide_all(&mut self) {
        self.hidden.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    pub data_dir: String,
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

/// Migrates an `AppState` instance across application version bumps, normalizing legacy
/// configurations, ensuring mandatory components are enabled, and populating new defaults.
pub fn migrate_app_state(state: &mut AppState, from_version: Option<&str>, target_version: &str) -> bool {
    let mut changed = false;
    let old_v = from_version.unwrap_or("legacy/unversioned");
    crate::core::logger::info("state", &format!("Checking library.json migration: {} -> {}", old_v, target_version));

    // 1. Normalize path separators across all cached games and executables
    for game in &mut state.cached_games {
        let cleaned_dir = clean_path_separators(&game.dir);
        if cleaned_dir != game.dir {
            game.dir = cleaned_dir;
            changed = true;
        }
        let cleaned_exe = clean_path_separators(&game.exe_path);
        if cleaned_exe != game.exe_path {
            game.exe_path = cleaned_exe;
            changed = true;
        }
        for opt in &mut game.available_exes {
            let cleaned_opt = clean_path_separators(&opt.path);
            if cleaned_opt != opt.path {
                opt.path = cleaned_opt;
                changed = true;
            }
        }
    }

    // 2. Normalize manual folders and excluded roots
    for folder in &mut state.folders {
        let cleaned = clean_path_separators(std::path::Path::new(folder)).to_string_lossy().to_string();
        if cleaned != *folder {
            *folder = cleaned;
            changed = true;
        }
    }
    for root in &mut state.excluded_roots {
        let cleaned = clean_path_separators(std::path::Path::new(root)).to_string_lossy().to_string();
        if cleaned != *root {
            *root = cleaned;
            changed = true;
        }
    }

    // 3. Ensure mandatory core engine add-ons are enabled
    for mandatory in &["builtin:renodx", "builtin:mfgunlock", "builtin:feeder"] {
        if !state.addons.iter().any(|a| a == *mandatory) {
            state.addons.push(mandatory.to_string());
            changed = true;
        }
    }

    // 4. Always stamp with target version
    if state.version.as_deref() != Some(target_version) {
        state.version = Some(target_version.to_string());
        changed = true;
    }

    changed
}

pub fn load_state() -> AppState {
    let path = get_state_path();
    if !path.exists() {
        // First run: initialize default state without altering system startup registry
        let default_st = AppState::default();
        let _ = save_state(&default_st);
        return default_st;
    }
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(mut state) = serde_json::from_str::<AppState>(&content) {
            let current_version = CURRENT_APP_VERSION;
            let old_version = state.version.clone();
            let needs_migration = old_version.as_deref() != Some(current_version);
            let migrated = migrate_app_state(&mut state, old_version.as_deref(), current_version);
            if needs_migration || migrated {
                let _ = save_state(&state);
            }
            return state;
        }
    }
    AppState::default()
}

pub fn save_state(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut to_save = state.clone();
    if to_save.version.as_deref() != Some(CURRENT_APP_VERSION) {
        to_save.version = Some(CURRENT_APP_VERSION.to_string());
    }
    let json = serde_json::to_string_pretty(&to_save)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn touch(dir: &str) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut state = load_state();
    state.recents.retain(|r| r.dir.to_lowercase() != dir.to_lowercase());
    state.recents.insert(0, RecentEntry { dir: dir.to_string(), at: now });
    if state.recents.len() > 12 {
        state.recents.truncate(12);
    }
    let _ = save_state(&state);
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


pub fn is_addon_active(state: &AppState, id: &str) -> bool {
    // Base add-ons are mandatory core engine components and always active
    if id == "builtin:renodx" || id == "builtin:mfgunlock" || id == "builtin:feeder" {
        return true;
    }
    // Overlay is shelved
    if id == "builtin:overlay" {
        return false;
    }
    state.addons.iter().any(|a| a == id)
}

#[doc(hidden)]
pub fn toggle_addon_in_state(state: &mut AppState, id: &str, active: bool) {
    // Base add-ons are mandatory and cannot be deactivated
    if id == "builtin:renodx" || id == "builtin:mfgunlock" || id == "builtin:feeder" {
        if !state.addons.iter().any(|a| a == id) {
            state.addons.push(id.to_string());
        }
        return;
    }
    // Overlay cannot be enabled
    if id == "builtin:overlay" {
        return;
    }
    if active {
        if !state.addons.iter().any(|a| a == id) {
            state.addons.push(id.to_string());
        }
    } else {
        state.addons.retain(|a| a != id);
    }
}

#[doc(hidden)]
pub fn add_custom_addon(state: &mut AppState, entry: AddonFileEntry) {
    state.addon_files.retain(|e| e.path != entry.path);
    if !state.addons.contains(&entry.path) {
        state.addons.push(entry.path.clone());
    }
    state.addon_files.push(entry);
}

#[doc(hidden)]
pub fn remove_custom_addon(state: &mut AppState, path: &str) {
    state.addon_files.retain(|e| e.path != path);
    state.addons.retain(|a| a != path);
}

pub fn sync_overlay_preferences(state: &AppState) {
    let appdata = get_appdata_dir();
    let pref_file = appdata.join("overlay-preferences.json");
    let hotkey_code = match state.overlay_hotkey.to_uppercase().as_str() {
        "F1" => 112, "F2" => 113, "F3" => 114, "F4" => 115, "F5" => 116,
        "F6" => 117, "F7" => 118, "F8" => 119, "F9" => 120, "F10" => 121,
        "F11" => 122, "F12" => 123, _ => 119,
    };
    let theme_str = match state.overlay_theme.as_str() {
        "blue" | "azure" => "azure",
        "purple" | "amethyst" => "amethyst",
        _ => "emerald",
    };
    let payload = serde_json::json!({
        "hotkey": hotkey_code,
        "theme": theme_str,
        "enabled": state.overlay_enabled,
    });
    let _ = fs::write(&pref_file, payload.to_string());
}


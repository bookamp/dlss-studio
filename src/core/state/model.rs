//! Application state data models, serde schema definitions, and defaults.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use super::paths::{normalize_game_path, normalize_path_str};

pub const CURRENT_APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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

pub fn default_addons() -> Vec<String> {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    pub data_dir: String,
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

pub fn default_theme() -> String { "dark".to_string() }
pub fn default_lang() -> String { crate::core::i18n::detect_system_language() }
pub fn default_true() -> bool { true }
pub fn default_false() -> bool { false }
pub fn default_overlay_theme() -> String { "green".to_string() }
pub fn default_overlay_hotkey() -> String { "F8".to_string() }

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

impl AppState {
    pub fn get_custom_name(&self, dir: &Path) -> Option<&String> {
        let norm = normalize_game_path(dir);
        self.custom_names.get(&norm)
    }

    pub fn set_custom_name(&mut self, dir: &Path, name: &str) {
        let norm = normalize_game_path(dir);
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.custom_names.remove(&norm);
        } else {
            self.custom_names.insert(norm, trimmed.to_string());
        }
    }

    pub fn is_hidden(&self, dir: &Path) -> bool {
        let norm = normalize_game_path(dir);
        self.hidden.iter().any(|h| normalize_path_str(h) == norm)
    }

    pub fn hide_game(&mut self, dir: &Path) {
        let norm = normalize_game_path(dir);
        if !self.hidden.iter().any(|h| normalize_path_str(h) == norm) {
            self.hidden.push(dir.to_string_lossy().to_string());
        }
        self.manual.retain(|m| normalize_path_str(m) != norm);
        self.cached_games.retain(|g| normalize_game_path(&g.dir) != norm);
    }

    pub fn unhide_game(&mut self, dir: &Path) {
        let norm = normalize_game_path(dir);
        self.hidden.retain(|h| normalize_path_str(h) != norm);
    }

    pub fn unhide_all(&mut self) {
        self.hidden.clear();
    }
}

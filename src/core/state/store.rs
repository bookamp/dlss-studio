//! Persistence, state version migration, recents management, and add-on toggling.

use std::fs;

use super::model::{AddonFileEntry, AppState, RecentEntry, CURRENT_APP_VERSION};
use super::paths::{clean_path_separators, get_appdata_dir, get_state_path};

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

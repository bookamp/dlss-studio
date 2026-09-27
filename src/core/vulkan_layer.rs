use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
#[cfg(not(test))]
use windows::core::PCWSTR;
#[cfg(not(test))]
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE,
};

const VULKAN_IMPLICIT_LAYERS_SUBKEY: &str = "Software\\Khronos\\Vulkan\\ImplicitLayers";

#[derive(Debug, Serialize, Deserialize, Default)]
struct InstallsRecord {
    version: u32,
    games: Vec<String>,
}

#[cfg(test)]
thread_local! {
    static TEST_STORAGE_DIR: PathBuf = {
        let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("dlss_test_vk_{}_{:?}", id, std::thread::current().id()));
        let _ = fs::create_dir_all(&p);
        p
    };
}

#[doc(hidden)]
pub fn get_vulkan_layer_storage_dir() -> PathBuf {
    #[cfg(test)]
    {
        TEST_STORAGE_DIR.with(|p| p.clone())
    }
    #[cfg(not(test))]
    {
        let p = crate::core::state::get_appdata_dir().join("vulkan_layer");
        let _ = fs::create_dir_all(&p);
        p
    }
}

#[doc(hidden)]
pub fn is_valid_64bit_pe(path: &Path) -> bool {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() < 100_000 {
            return false;
        }
        if let Some(pe) = crate::core::pe::inspect_pe(path) {
            return pe.bitness == 64;
        }
    }
    false
}

#[doc(hidden)]
pub fn read_registered_games(storage_dir: &Path) -> Vec<String> {
    let rec_path = storage_dir.join("installs.json");
    if let Ok(data) = fs::read_to_string(&rec_path) {
        if let Ok(rec) = serde_json::from_str::<InstallsRecord>(&data) {
            return rec.games;
        }
    }
    Vec::new()
}

#[doc(hidden)]
pub fn save_registered_games(storage_dir: &Path, games: &[String]) -> std::io::Result<()> {
    let rec_path = storage_dir.join("installs.json");
    let rec = InstallsRecord {
        version: 1,
        games: games.to_vec(),
    };
    let json = serde_json::to_string_pretty(&rec).unwrap_or_default();
    fs::write(rec_path, json)
}

#[cfg(not(test))]
fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
fn set_registry_dword(_subkey: &str, _value_name: &str, _dword_val: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(not(test))]
fn set_registry_dword(subkey: &str, value_name: &str, dword_val: u32) -> Result<(), String> {
    unsafe {
        let wide_sub = to_wide_null(subkey);
        let mut hkey: HKEY = HKEY::default();
        let res = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if res.is_err() {
            return Err(format!("Failed to open/create registry key HKCU\\{}: {:?}", subkey, res));
        }

        let wide_val = to_wide_null(value_name);
        let bytes = dword_val.to_ne_bytes();
        let set_res = RegSetValueExW(
            hkey,
            PCWSTR::from_raw(wide_val.as_ptr()),
            0,
            REG_DWORD,
            Some(&bytes),
        );
        let _ = RegCloseKey(hkey);

        if set_res.is_err() {
            return Err(format!("Failed to set registry value {}: {:?}", value_name, set_res));
        }
    }
    Ok(())
}

#[cfg(test)]
fn delete_registry_value(_subkey: &str, _value_name: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(test))]
fn delete_registry_value(subkey: &str, value_name: &str) -> Result<(), String> {
    unsafe {
        let wide_sub = to_wide_null(subkey);
        let mut hkey: HKEY = HKEY::default();
        let res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(wide_sub.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        );
        if res.is_err() {
            return Ok(()); // Key doesn't exist
        }

        let wide_val = to_wide_null(value_name);
        let _ = RegDeleteValueW(hkey, PCWSTR::from_raw(wide_val.as_ptr()));
        let _ = RegCloseKey(hkey);
    }
    Ok(())
}

/// Registers the Vulkan implicit layer for a target game.
/// Supports both DLSS5-Feeder VkLayer_feed_vk and ReShade64 layer.
pub fn register_vulkan_layer(
    game_dir: &Path,
    layer_source_dir: Option<&Path>,
    reshade_dll_fallback: Option<&Path>,
) -> Result<PathBuf, String> {
    let storage_dir = get_vulkan_layer_storage_dir();
    let norm_game_dir = game_dir.canonicalize().unwrap_or_else(|_| game_dir.to_path_buf());
    let norm_str = norm_game_dir.to_string_lossy().to_string();

    let (json_name, dll_name) = if let Some(src) = layer_source_dir {
        if src.join("VkLayer_feed_vk.dll").is_file() {
            ("VkLayer_feed_vk.json", "VkLayer_feed_vk.dll")
        } else {
            ("ReShade64.json", "ReShade64.dll")
        }
    } else {
        ("ReShade64.json", "ReShade64.dll")
    };

    let target_json = storage_dir.join(json_name);
    let target_dll = storage_dir.join(dll_name);

    // Copy source DLL and JSON if provided
    let mut dll_valid = is_valid_64bit_pe(&target_dll);
    if let Some(src) = layer_source_dir {
        let src_dll = src.join(dll_name);
        let src_json = src.join(json_name);
        if src_dll.is_file() && (!dll_valid || is_valid_64bit_pe(&src_dll)) {
            if fs::copy(&src_dll, &target_dll).is_ok() {
                dll_valid = is_valid_64bit_pe(&target_dll);
            }
        }
        if src_json.is_file() {
            if let Ok(mut content) = fs::read_to_string(&src_json) {
                // Point library_path to local DLL
                if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(layer) = parsed.get_mut("layer") {
                        if layer.is_object() {
                            layer["library_path"] = serde_json::Value::String(target_dll.to_string_lossy().to_string());
                        }
                    }
                    content = serde_json::to_string_pretty(&parsed).unwrap_or(content);
                }
                let _ = fs::write(&target_json, content);
            }
        }
    }

    // If target DLL not yet copied or invalid, copy from reshade_dll_fallback
    if !dll_valid {
        if let Some(fallback) = reshade_dll_fallback {
            if fallback.is_file() {
                let _ = fs::copy(fallback, &target_dll);
                dll_valid = is_valid_64bit_pe(&target_dll);
            }
        }
    }
    let _ = &dll_valid;

    #[cfg(not(test))]
    if !dll_valid && !target_dll.is_file() {
        return Err(format!(
            "Vulkan layer DLL is missing: {}",
            target_dll.display()
        ));
    }

    // If target JSON does not exist, generate standard Khronos Vulkan layer manifest
    if !target_json.is_file() {
        let layer_name = if dll_name.contains("feed_vk") {
            "VK_LAYER_DLSS5_Feed"
        } else {
            "VK_LAYER_reshade"
        };
        let desc = if dll_name.contains("feed_vk") {
            "DLSS 5 Feeder Vulkan Layer"
        } else {
            "ReShade Vulkan Layer"
        };

        let val = serde_json::json!({
            "file_format_version": "1.0.0",
            "layer": {
                "name": layer_name,
                "type": "GLOBAL",
                "library_path": target_dll.to_string_lossy().to_string(),
                "api_version": "1.3.0",
                "implementation_version": "1",
                "description": desc,
                "disable_environment": {
                    "DISABLE_RESHADE": "1"
                }
            }
        });
        if let Ok(json_str) = serde_json::to_string_pretty(&val) {
            let _ = fs::write(&target_json, json_str);
        }
    }

    if !target_json.is_file() {
        return Err(format!("Vulkan layer manifest not found at {}", target_json.display()));
    }

    let json_str = target_json.to_string_lossy().to_string();
    set_registry_dword(VULKAN_IMPLICIT_LAYERS_SUBKEY, &json_str, 0)?;

    let mut games = read_registered_games(&storage_dir);
    if !games.iter().any(|g| g.eq_ignore_ascii_case(&norm_str)) {
        games.push(norm_str);
        let _ = save_registered_games(&storage_dir, &games);
    }

    crate::core::logger::info("vulkan_layer", &format!("Registered Vulkan implicit layer manifest: {}", json_str));
    Ok(target_json)
}

/// Unregisters the Vulkan implicit layer for a game.
pub fn unregister_vulkan_layer(game_dir: &Path) -> Result<bool, String> {
    let storage_dir = get_vulkan_layer_storage_dir();
    let norm_game_dir = game_dir.canonicalize().unwrap_or_else(|_| game_dir.to_path_buf());
    let norm_str = norm_game_dir.to_string_lossy().to_string();

    let mut games = read_registered_games(&storage_dir);
    games.retain(|g| !g.eq_ignore_ascii_case(&norm_str));
    let _ = save_registered_games(&storage_dir, &games);

    // If other games are still using this layer, do not delete the registry entry
    if !games.is_empty() {
        return Ok(false);
    }

    for json_name in &["VkLayer_feed_vk.json", "ReShade64.json"] {
        let target_json = storage_dir.join(json_name);
        let json_str = target_json.to_string_lossy().to_string();
        let _ = delete_registry_value(VULKAN_IMPLICIT_LAYERS_SUBKEY, &json_str);
    }

    crate::core::logger::info("vulkan_layer", "Unregistered Vulkan implicit layer from registry");
    Ok(true)
}

/// Checks if a game directory is currently registered for the Vulkan implicit layer.
#[allow(dead_code)]
pub fn is_game_registered(game_dir: &Path) -> bool {
    let storage_dir = get_vulkan_layer_storage_dir();
    let norm_game_dir = game_dir.canonicalize().unwrap_or_else(|_| game_dir.to_path_buf());
    let norm_str = norm_game_dir.to_string_lossy().to_string();
    let games = read_registered_games(&storage_dir);
    games.iter().any(|g| g.eq_ignore_ascii_case(&norm_str))
}


//! Pure Rust Vibepollo / Apollo / Sunshine Integration Engine.
//!
//! Provides presence detection, apps.json parsing, and registration of DLSS Studio
//! in Big Picture mode with cover art and `--big-picture` launch arguments.

use std::path::{Path, PathBuf};
use std::fs;
use crate::core::install_guards::get_running_processes;

pub const DEFAULT_SUNSHINE_DIR: &str = r"C:\Program Files\Sunshine";
pub const DEFAULT_APPS_JSON: &str = r"C:\Program Files\Sunshine\config\apps.json";
pub const DLSS_STUDIO_APP_NAME: &str = "DLSS Studio";
pub const DLSS_STUDIO_UUID: &str = "4C640001-A480-4D56-9132-DLSS5STUDIO2";

/// Checks whether Vibepollo / Apollo / Sunshine is installed and available on the system.
pub fn is_vibepollo_installed() -> bool {
    // 1. Check standard config or binary paths
    let apps_path = Path::new(DEFAULT_APPS_JSON);
    if apps_path.exists() {
        return true;
    }

    let exe_path = Path::new(DEFAULT_SUNSHINE_DIR).join("sunshine.exe");
    let svc_path = Path::new(DEFAULT_SUNSHINE_DIR).join("tools").join("sunshinesvc.exe");
    if exe_path.exists() || svc_path.exists() {
        return true;
    }

    // 2. Check running processes
    let running = get_running_processes();
    if running.iter().any(|p| {
        let l = p.name.to_lowercase();
        l.contains("sunshine") || l.contains("apollo") || l.contains("vibepollo")
    }) {
        return true;
    }

    // 3. Check Windows Registry (LizardByte / Sunshine or Apollo)
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{
            RegOpenKeyExW, HKEY_LOCAL_MACHINE, KEY_READ, HKEY,
        };
        use windows::core::PCWSTR;

        unsafe {
            let key_name: Vec<u16> = "SOFTWARE\\LizardByte\\Sunshine\0".encode_utf16().collect();
            let mut hkey = HKEY::default();
            let res = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(key_name.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            );
            if res.is_ok() {
                let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
                return true;
            }
        }
    }

    false
}

/// Resolves the active `apps.json` path for Vibepollo / Sunshine.
pub fn get_vibepollo_apps_path() -> Option<PathBuf> {
    let p = PathBuf::from(DEFAULT_APPS_JSON);
    if p.exists() {
        return Some(p);
    }

    if let Ok(progdata) = std::env::var("ProgramData") {
        let p_data = PathBuf::from(progdata).join("Sunshine").join("config").join("apps.json");
        if p_data.exists() {
            return Some(p_data);
        }
    }

    if is_vibepollo_installed() {
        return Some(p);
    }

    None
}

/// Checks whether DLSS Studio is currently registered in `apps.json`.
#[allow(dead_code)]
pub fn is_app_registered() -> bool {
    let apps_path = match get_vibepollo_apps_path() {
        Some(p) => p,
        None => return false,
    };

    if !apps_path.exists() {
        return false;
    }

    let content = match fs::read_to_string(&apps_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    if let Some(apps) = json.get("apps").and_then(|a| a.as_array()) {
        apps.iter().any(|app| {
            app.get("name").and_then(|n| n.as_str()) == Some(DLSS_STUDIO_APP_NAME)
        })
    } else {
        false
    }
}

/// Prepares and exports the high-resolution 600x900 vertical poster image to the local AppData folder.
pub fn ensure_vibepollo_cover_art() -> Option<PathBuf> {
    let art_dir = crate::core::state::get_appdata_dir().join("art");
    let _ = fs::create_dir_all(&art_dir);
    let poster_path = art_dir.join("vibepollo_poster_v2.png");
    let legacy_cover = art_dir.join("vibepollo_cover.png");
    if legacy_cover.exists() {
        let _ = fs::remove_file(legacy_cover);
    }

    let poster_bytes = include_bytes!("../../assets/vibepollo_poster.webp");
    let needs_update = if !poster_path.exists() {
        true
    } else {
        match fs::read(&poster_path) {
            Ok(existing_bytes) => {
                // Check for valid PNG magic bytes and reasonable size (> 50 KB)
                existing_bytes.len() < 50_000 || &existing_bytes[..8.min(existing_bytes.len())] != b"\x89PNG\r\n\x1a\n"
            }
            Err(_) => true,
        }
    };

    if needs_update {
        if let Ok(img) = image::load_from_memory_with_format(poster_bytes, image::ImageFormat::WebP) {
            let _ = img.save_with_format(&poster_path, image::ImageFormat::Png);
        } else {
            let _ = fs::write(&poster_path, poster_bytes);
        }
    }

    if poster_path.exists() {
        Some(poster_path)
    } else {
        None
    }
}

/// Adds or updates DLSS Studio in Vibepollo's `apps.json`.
pub fn register_app() -> Result<(), String> {
    let apps_path = get_vibepollo_apps_path()
        .ok_or_else(|| "Vibepollo apps.json path could not be located".to_string())?;

    let curr_exe = std::env::current_exe()
        .map_err(|e| format!("Could not resolve current executable path: {}", e))?;
    let working_dir = curr_exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let exe_str = curr_exe.to_string_lossy().to_string();

    let cover_art = ensure_vibepollo_cover_art()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut json = if apps_path.exists() {
        let content = fs::read_to_string(&apps_path)
            .map_err(|e| format!("Failed to read {}: {}", apps_path.display(), e))?;
        serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|e| format!("Failed to parse apps.json: {}", e))?
    } else {
        serde_json::json!({
            "apps": [],
            "env": {},
            "version": 2
        })
    };

    let apps_array = json
        .get_mut("apps")
        .and_then(|a| a.as_array_mut())
        .ok_or_else(|| "Invalid apps.json structure: missing 'apps' array".to_string())?;

    let dlss_app_entry = serde_json::json!({
        "name": DLSS_STUDIO_APP_NAME,
        "detached": [
            format!("\"{}\" --big-picture", exe_str)
        ],
        "working-dir": working_dir,
        "image-path": cover_art,
        "uuid": DLSS_STUDIO_UUID,
        "continue-streaming-if-application-exits-quickly": true,
        "continue-streaming-until-all-app-processes-exit": true,
        "allow-client-commands": true,
        "elevated": false
    });

    if let Some(pos) = apps_array.iter().position(|a| {
        a.get("name").and_then(|n| n.as_str()) == Some(DLSS_STUDIO_APP_NAME)
            || a.get("uuid").and_then(|u| u.as_str()) == Some("4C640001-A480-4D56-9132-DLSS5STUDIO1")
            || a.get("uuid").and_then(|u| u.as_str()) == Some(DLSS_STUDIO_UUID)
    }) {
        apps_array[pos] = dlss_app_entry;
    } else {
        apps_array.push(dlss_app_entry);
    }

    let formatted_json = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("Failed to format JSON: {}", e))?;

    write_apps_json(&apps_path, &formatted_json)
}

/// Removes DLSS Studio from Vibepollo's `apps.json`.
pub fn unregister_app() -> Result<(), String> {
    let apps_path = match get_vibepollo_apps_path() {
        Some(p) => p,
        None => return Ok(()), // Nothing to unregister
    };

    if !apps_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&apps_path)
        .map_err(|e| format!("Failed to read {}: {}", apps_path.display(), e))?;

    let mut json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse apps.json: {}", e))?;

    if let Some(apps_array) = json.get_mut("apps").and_then(|a| a.as_array_mut()) {
        let initial_len = apps_array.len();
        apps_array.retain(|a| {
            a.get("name").and_then(|n| n.as_str()) != Some(DLSS_STUDIO_APP_NAME)
                && a.get("uuid").and_then(|u| u.as_str()) != Some("4C640001-A480-4D56-9132-DLSS5STUDIO1")
                && a.get("uuid").and_then(|u| u.as_str()) != Some(DLSS_STUDIO_UUID)
        });

        if apps_array.len() == initial_len {
            return Ok(()); // Already not present
        }
    }

    let formatted_json = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("Failed to format JSON: {}", e))?;

    write_apps_json(&apps_path, &formatted_json)
}

/// Writes updated content to `apps.json`, attempting direct write first and
/// falling back to Windows UAC elevation (`runas`) if permission is denied.
fn write_apps_json(target_path: &Path, content: &str) -> Result<(), String> {
    // 1. Direct write attempt
    if let Some(parent) = target_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    match fs::write(target_path, content) {
        Ok(_) => return Ok(()),
        Err(e) if e.kind() != std::io::ErrorKind::PermissionDenied => {
            return Err(format!("Failed to write to {}: {}", target_path.display(), e));
        }
        Err(_) => {
            // Permission denied: proceed to elevated write
        }
    }

    // 2. Elevated UAC write via temporary file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("vibepollo_apps_{}.json", std::process::id()));

    fs::write(&temp_file, content)
        .map_err(|e| format!("Failed to write temporary elevated staging file: {}", e))?;

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
        use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
        use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
        use windows::core::PCWSTR;

        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        let file: Vec<u16> = "cmd.exe\0".encode_utf16().collect();
        let params_str = format!(
            "/c copy /y \"{}\" \"{}\"\0",
            temp_file.display(),
            target_path.display()
        );
        let params: Vec<u16> = params_str.encode_utf16().collect();

        let mut exec_info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS,
            hwnd: windows::Win32::Foundation::HWND::default(),
            lpVerb: PCWSTR::from_raw(verb.as_ptr()),
            lpFile: PCWSTR::from_raw(file.as_ptr()),
            lpParameters: PCWSTR::from_raw(params.as_ptr()),
            lpDirectory: PCWSTR::null(),
            nShow: SW_HIDE.0 as i32,
            ..Default::default()
        };

        let res = unsafe { ShellExecuteExW(&mut exec_info) };

        if res.is_err() || exec_info.hProcess.is_invalid() {
            let _ = fs::remove_file(&temp_file);
            return Err("Administrator permissions were cancelled or denied.".to_string());
        }

        // Wait up to 10 seconds for the elevated copy command to finish
        let wait_res = unsafe { WaitForSingleObject(exec_info.hProcess, 10_000) };
        let mut exit_code: u32 = 1;
        unsafe {
            let _ = GetExitCodeProcess(exec_info.hProcess, &mut exit_code);
            let _ = CloseHandle(exec_info.hProcess);
        }

        let _ = fs::remove_file(&temp_file);

        if wait_res.0 != 0 || exit_code != 0 {
            return Err(format!("Elevated write failed with exit code {}", exit_code));
        }

        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = fs::remove_file(&temp_file);
        Err("Vibepollo UAC elevation is only supported on Windows".to_string())
    }
}


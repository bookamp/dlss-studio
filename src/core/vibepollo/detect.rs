//! Service and registry presence detection for Vibepollo, Apollo, and Sunshine.

use std::path::{Path, PathBuf};
use crate::core::install_guards::get_running_processes;

pub const DEFAULT_APOLLO_DIR: &str = r"C:\Program Files\Apollo";
pub const DEFAULT_APOLLO_APPS_JSON: &str = r"C:\Program Files\Apollo\config\apps.json";
pub const DEFAULT_SUNSHINE_DIR: &str = r"C:\Program Files\Sunshine";
pub const DEFAULT_SUNSHINE_APPS_JSON: &str = r"C:\Program Files\Sunshine\config\apps.json";
pub const DEFAULT_APPS_JSON: &str = DEFAULT_APOLLO_APPS_JSON;

/// Parses a Windows Service `ImagePath` command-line string to extract the application root directory.
///
/// Handles enclosing quotes, command-line arguments (e.g., `--service`), and automatically unwraps
/// intermediate subfolders such as `tools\` or `bin\` to reach the canonical installation root.
pub fn parse_service_install_root(image_path: &str) -> Option<PathBuf> {
    let trimmed = image_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let exe_path_str = if trimmed.starts_with('"') {
        if let Some(end) = trimmed[1..].find('"') {
            &trimmed[1..1 + end]
        } else {
            trimmed.trim_matches('"')
        }
    } else if let Some(exe_idx) = trimmed.to_lowercase().find(".exe") {
        &trimmed[..exe_idx + 4]
    } else {
        trimmed.split_whitespace().next().unwrap_or(trimmed)
    };

    let p = PathBuf::from(exe_path_str);
    let parent = p.parent()?;

    // If the binary resides in a "tools" or "bin" subfolder, its parent is the installation root
    if let Some(dir_name) = parent.file_name().and_then(|n| n.to_str()) {
        let l = dir_name.to_lowercase();
        if (l == "tools" || l == "bin") && parent.parent().is_some() {
            return parent.parent().map(|gp| gp.to_path_buf());
        }
    }

    Some(parent.to_path_buf())
}

/// Helper to expand Windows environment variables (e.g. `%ProgramFiles%`, `%SystemRoot%`).
#[cfg(windows)]
pub fn expand_env_vars(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }

    let mut result = s.to_string();
    for var in &[
        "ProgramFiles",
        "ProgramFiles(x86)",
        "SystemRoot",
        "SystemDrive",
        "ProgramData",
        "LOCALAPPDATA",
    ] {
        if let Ok(val) = std::env::var(var) {
            let token = format!("%{}%", var);
            // Replace case-insensitively
            while let Some(pos) = result.to_lowercase().find(&token.to_lowercase()) {
                result.replace_range(pos..pos + token.len(), &val);
            }
        }
    }
    result
}

/// Reads a string value from HKLM registry.
#[cfg(windows)]
pub fn read_hklm_reg_string(subkey: &str, value_name: &str) -> Option<String> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE, KEY_READ, HKEY,
        REG_VALUE_TYPE,
    };
    use windows::core::PCWSTR;

    let subkey_wide: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let val_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = HKEY::default();

    unsafe {
        let res = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );
        if res.is_err() || hkey.is_invalid() {
            return None;
        }

        let mut val_type = REG_VALUE_TYPE::default();
        let mut byte_len: u32 = 0;
        let query_res = RegQueryValueExW(
            hkey,
            PCWSTR(val_wide.as_ptr()),
            None,
            Some(&mut val_type),
            None,
            Some(&mut byte_len),
        );

        if query_res.is_err() || byte_len == 0 {
            let _ = RegCloseKey(hkey);
            return None;
        }

        let num_u16 = (byte_len as usize + 1) / 2;
        let mut buffer: Vec<u16> = vec![0u16; num_u16];
        let query_val = RegQueryValueExW(
            hkey,
            PCWSTR(val_wide.as_ptr()),
            None,
            Some(&mut val_type),
            Some(buffer.as_mut_ptr() as *mut u8),
            Some(&mut byte_len),
        );

        let _ = RegCloseKey(hkey);

        if query_val.is_ok() {
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            let raw_str = String::from_utf16(&buffer[..len]).ok()?;
            Some(expand_env_vars(&raw_str))
        } else {
            None
        }
    }
}

/// Inspects registered Windows Services and Uninstall registry keys to discover the root installation path.
pub fn get_service_install_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // 1. Direct query of known service registry keys (O(1) lookups)
        const KNOWN_SERVICES: &[&str] = &[
            "ApolloService",
            "SunshineService",
            "sunshinesvc",
            "VibepolloService",
            "Vibepollo",
        ];

        for &svc in KNOWN_SERVICES {
            let key = format!(r"SYSTEM\CurrentControlSet\Services\{}", svc);
            if let Some(image_path) = read_hklm_reg_string(&key, "ImagePath") {
                if let Some(root) = parse_service_install_root(&image_path) {
                    if root.exists() {
                        return Some(root);
                    }
                }
            }
        }

        // 2. Query Windows Uninstall registry entries (Vibepollo / Apollo / Sunshine)
        const UNINSTALL_KEYS: &[&str] = &[
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Vibepollo",
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Apollo",
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Sunshine",
        ];
        for &uninst in UNINSTALL_KEYS {
            if let Some(loc) = read_hklm_reg_string(uninst, "InstallLocation") {
                let p = PathBuf::from(loc.trim_matches('"'));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// Checks whether Vibepollo / Apollo / Sunshine is installed and available on the system.
pub fn is_vibepollo_installed() -> bool {
    // 1. Check if a Windows Service or registered installation exists
    if get_service_install_root().is_some() {
        return true;
    }

    // 2. Check standard config or binary paths
    let standard_candidates = [
        r"C:\Program Files\Apollo\config\apps.json",
        r"C:\Program Files\Vibepollo\config\apps.json",
        r"C:\Program Files\Sunshine\config\apps.json",
    ];
    if standard_candidates.iter().any(|c| Path::new(c).exists()) {
        return true;
    }

    let standard_dirs = [
        DEFAULT_APOLLO_DIR,
        r"C:\Program Files\Vibepollo",
        DEFAULT_SUNSHINE_DIR,
    ];
    if standard_dirs.iter().any(|d| {
        let p = Path::new(d);
        p.join("sunshine.exe").exists()
            || p.join("apollo.exe").exists()
            || p.join("tools").join("sunshinesvc.exe").exists()
    }) {
        return true;
    }

    // 3. Check running processes
    let running = get_running_processes();
    if running.iter().any(|p| {
        let l = p.name.to_lowercase();
        l.contains("sunshine") || l.contains("apollo") || l.contains("vibepollo")
    }) {
        return true;
    }

    // 4. Check Windows Registry (LizardByte / Sunshine or Apollo)
    #[cfg(windows)]
    {
        use windows::Win32::System::Registry::{
            RegCloseKey, RegOpenKeyExW, HKEY_LOCAL_MACHINE, KEY_READ, HKEY,
        };
        use windows::core::PCWSTR;

        unsafe {
            for key_str in &[
                "SOFTWARE\\LizardByte\\Sunshine\0",
                "SOFTWARE\\Vibepollo\0",
                "SOFTWARE\\Apollo\0",
            ] {
                let key_name: Vec<u16> = key_str.encode_utf16().collect();
                let mut hkey = HKEY::default();
                let res = RegOpenKeyExW(
                    HKEY_LOCAL_MACHINE,
                    PCWSTR::from_raw(key_name.as_ptr()),
                    0,
                    KEY_READ,
                    &mut hkey,
                );
                if res.is_ok() {
                    let _ = RegCloseKey(hkey);
                    return true;
                }
            }
        }
    }

    false
}

/// Resolves the active `apps.json` path for Vibepollo / Apollo / Sunshine.
///
/// Prioritizes the active Windows Service root (`ImagePath`), followed by existing
/// candidate installation folders on disk, `%ProgramData%` configurations, and fallbacks.
pub fn get_vibepollo_apps_path() -> Option<PathBuf> {
    // 1. Service-first lookup (authoritative for active running host)
    if let Some(service_root) = get_service_install_root() {
        let app_json = service_root.join("config").join("apps.json");
        return Some(app_json);
    }

    // 2. Existing candidate paths in standard installation locations
    let candidates = [
        DEFAULT_APOLLO_APPS_JSON,
        r"C:\Program Files\Vibepollo\config\apps.json",
        DEFAULT_SUNSHINE_APPS_JSON,
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }

    // 3. ProgramData configuration paths
    if let Ok(progdata) = std::env::var("ProgramData") {
        for sub in &["Apollo", "Vibepollo", "Sunshine"] {
            let p = PathBuf::from(&progdata).join(sub).join("config").join("apps.json");
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 4. Candidate directories that exist on disk (even if apps.json has not yet been initialized)
    for root in &[DEFAULT_APOLLO_DIR, r"C:\Program Files\Vibepollo", DEFAULT_SUNSHINE_DIR] {
        let r = Path::new(root);
        if r.exists() {
            return Some(r.join("config").join("apps.json"));
        }
    }

    // 5. Default fallback if installed/running
    if is_vibepollo_installed() {
        return Some(PathBuf::from(DEFAULT_APOLLO_APPS_JSON));
    }

    None
}

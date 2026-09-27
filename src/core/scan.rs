use std::sync::LazyLock;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::core::pe::{inspect_pe, find_markers};
use regex::Regex;
#[cfg(windows)]
use windows::Win32::System::Registry::{
    RegOpenKeyExW, RegQueryValueExW, RegEnumKeyExW, RegCloseKey,
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY,
    REG_SAM_FLAGS, REG_VALUE_TYPE,
};
#[cfg(windows)]
use windows::core::{PCWSTR, PWSTR};

#[cfg(windows)]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn win32_read_reg_string(root: HKEY, subkey: &str, value_name: &str, sam: REG_SAM_FLAGS) -> Option<String> {
    let subkey_wide = to_wide(subkey);
    let val_wide = to_wide(value_name);
    let mut hkey = HKEY::default();

    unsafe {
        let res = RegOpenKeyExW(root, PCWSTR(subkey_wide.as_ptr()), 0, sam, &mut hkey);
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
            return String::from_utf16(&buffer[..len]).ok();
        }
    }
    None
}

#[cfg(windows)]
fn win32_enum_subkeys(root: HKEY, subkey: &str, sam: REG_SAM_FLAGS) -> Vec<String> {
    let mut results = Vec::new();
    let subkey_wide = to_wide(subkey);
    let mut hkey = HKEY::default();

    unsafe {
        let res = RegOpenKeyExW(root, PCWSTR(subkey_wide.as_ptr()), 0, sam, &mut hkey);
        if res.is_err() || hkey.is_invalid() {
            return results;
        }

        let mut index = 0u32;
        loop {
            let mut name_buf = [0u16; 260];
            let mut name_len = name_buf.len() as u32;
            let status = RegEnumKeyExW(
                hkey,
                index,
                PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                PWSTR::null(),
                None,
                None,
            );

            if status.is_err() {
                break;
            }

            if let Ok(s) = String::from_utf16(&name_buf[..name_len as usize]) {
                results.push(s);
            }
            index += 1;
        }

        let _ = RegCloseKey(hkey);
    }
    results
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GameFileItem {
    pub rel: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GameExeOption {
    pub name: String,
    pub path: PathBuf,
    pub rel: String,
    pub api: String,
    pub bitness: u32,
    #[serde(default)]
    pub is_laa: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GameEntry {
    pub name: String,
    pub dir: PathBuf,
    pub exe_path: PathBuf,
    pub exe_rel: String,
    pub bitness: u32,
    #[serde(default)]
    pub is_laa: bool,
    pub api: String,
    pub dlss_version: Option<String>,
    pub has_frame_generation: bool,
    #[serde(default)]
    pub can_inject_fg: bool,
    pub optiscaler_installed: bool,
    pub optiscaler_presr: bool,
    pub optiscaler_passes: u32,
    pub mfg_unlock_installed: bool,
    pub has_backup: bool,
    pub launcher: String,
    pub poster: Option<String>,
    pub reshade_installed: bool,
    pub reshade_version: Option<String>,
    pub reshade_addon_support: bool,
    pub addon_installed: bool,
    pub installed_route: Option<String>,
    pub files: Vec<GameFileItem>,
    #[serde(default)]
    pub available_exes: Vec<GameExeOption>,
    #[serde(default)]
    pub nr_style: usize,
    #[serde(default)]
    pub nr_style_enabled: bool,
    #[serde(default = "default_mfg_multiplier")]
    pub mfg_multiplier: u32,
    #[serde(default)]
    pub has_anti_cheat: bool,
}

pub fn default_mfg_multiplier() -> u32 { 4 }

impl GameEntry {
    pub fn is_dlss5_patched(&self) -> bool {
        self.installed_route.is_some() || self.optiscaler_installed || self.reshade_installed
    }

    pub fn route_display_name(&self) -> &'static str {
        match self.installed_route.as_deref() {
            Some("feeder") => "Feeder · Neural Rendering",
            Some("native") => "Native D3D12",
            Some("optiscaler") => "OptiScaler",
            _ if self.optiscaler_installed => "OptiScaler",
            _ if self.reshade_installed => "ReShade",
            _ => "Vanilla",
        }
    }

    /// Detects if the game natively ships with an unmodded DLSS pipeline.
    /// Strictly detects nvngx_dlss.dll files deployed by DLSS Studio mods (Feeder or OptiScaler).
    pub fn has_native_dlss(&self) -> bool {
        if self.dlss_version.is_none() && !self.files.iter().any(|f| {
            let lower = f.rel.to_lowercase();
            lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll") || lower.ends_with("nvngx.dll")
        }) {
            return false;
        }

        // 1. Check active manifest in _DLSS5_Backup/manifest.json
        if let Some(manifest) = crate::core::journal::read_manifest(&self.dir) {
            let was_added = manifest.added.iter().any(|a| {
                let lower = a.to_lowercase();
                lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll") || lower.ends_with("nvngx.dll")
            });
            let was_replaced = manifest.replaced.iter().any(|r| {
                let lower = r.rel.to_lowercase();
                lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll") || lower.ends_with("nvngx.dll")
            });
            if was_added && !was_replaced {
                return false;
            }
        }

        // 2. Check latest done/reverted manifest in _DLSS5_Backup/manifest.json.done-*
        if let Some(done_manifest) = crate::core::journal::read_latest_done_manifest(&self.dir) {
            let was_added = done_manifest.added.iter().any(|a| {
                let lower = a.to_lowercase();
                lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll") || lower.ends_with("nvngx.dll")
            });
            let was_replaced = done_manifest.replaced.iter().any(|r| {
                let lower = r.rel.to_lowercase();
                lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll") || lower.ends_with("nvngx.dll")
            });
            if was_added && !was_replaced && (self.optiscaler_installed || self.installed_route.is_some()) {
                return false;
            }
        }

        true
    }

    /// Detects if the game contains any native upscaler pipeline (DLSS, FSR 2/3, XeSS, or Streamline).
    pub fn has_native_upscaler(&self) -> bool {
        if self.has_native_dlss() {
            return true;
        }

        self.files.iter().any(|f| {
            let lower = f.rel.to_lowercase();
            lower.ends_with("libxess.dll")
                || lower.contains("ffx_fsr2")
                || lower.contains("ffx_fsr3")
                || lower.contains("amd_fidelityfx")
                || lower.ends_with("sl.interposer.dll")
        })
    }
}

pub const DEFAULT_STORE_ORDER: &[&str] = &[
    "Steam",
    "Xbox",
    "Epic Games",
    "GOG",
    "Added by hand",
    "My folders",
];

pub fn matches_store_filter(launcher: &str, store: &str) -> bool {
    if store == "All" || store.is_empty() {
        return true;
    }
    match store {
        "Steam" => launcher.eq_ignore_ascii_case("steam"),
        "Xbox" => launcher.to_ascii_lowercase().contains("xbox"),
        "Epic Games" => launcher.to_ascii_lowercase().contains("epic"),
        "GOG" => launcher.to_ascii_lowercase().contains("gog"),
        "Added by hand" => {
            let l = launcher.to_ascii_lowercase();
            l.contains("manual") || l.contains("hand")
        }
        "My folders" => {
            let l = launcher.to_ascii_lowercase();
            l.contains("folder") || l == "my folders"
        }
        other => launcher.eq_ignore_ascii_case(other),
    }
}

pub fn short_version(v: &str) -> String {
    if let Some(stripped) = v.strip_suffix(".0") {
        stripped.to_string()
    } else {
        v.to_string()
    }
}

static RE_INSTALLER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(unins|setup|install|vcredist|vc_redist|dxsetup|dxwebsetup|oalinst|uninstall|crashreport|crashhandler|unitycrashhandler|unrealcefsubprocess|easyanticheat|eac|battleye|be_service|launcher|activation|patch|update|dotnetfx|touchup|rapidcrc|autorun|autoplay|quicksfv|readme|config|benchmark|report|helper|service|cleanup|modorganizer|redlauncher|skse\d*_loader|hlds |srcds |steamerrorreporter|dgvoodoocpl|dgvoodoo|reshade|optiscaler|dlss5-feed|specialk|skif|bg3modmanager|modmanager|vortex|fluffy|fomod|vpk|.*compiler|.*compile)").unwrap()
});

static RE_NOT_GAME_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(steamapps|gamesave|gamesaves|workshop|downloading|shadercache|cache|caches|temp|tmp|backup|_dlss5_backup|reshade-shaders|host64|optiscaler|saves?|savegames?|redist|_?commonredist|__installer|installers?|setup|dlc|mods?|tools?|node_modules|\.git|bg3modmanager.*|.*modmanager.*|vortex.*|fluffy.*|modorganizer.*|trainers?|cheats?|sdk|patcher)$").unwrap()
});

static RE_NOT_GAME_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(redistributabl|steamworks common|directx|vcredist|proton|steam linux runtime|soundtrack|modmanager|mod manager|save editor|trainer|cheat engine|nexus mods|sdk )").unwrap()
});

static RE_CONTAINER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(games?|my ?games|steamlibrary|gog ?games|gog|epic ?games|epic|xbox ?games|origin ?games|repacks?|emulation)$").unwrap()
});

pub fn is_installer_or_helper(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower == "gamelaunchhelper.exe" || lower.starts_with("gamelaunchhelper") 
        || lower == "dlss5-feed-host64.exe" || lower.starts_with("dlss5-feed") || lower.contains("feed-host")
        || lower.starts_with("unitycrashhandler") || lower.contains("crashhandler") || lower.contains("crashreport")
        || lower == "unrealcefsubprocess.exe" || lower.contains("cefsubprocess") || lower.contains("webhelper")
        || lower.starts_with("dgvoodoo") || lower.starts_with("reshade") || lower.starts_with("optiscaler")
        || lower.starts_with("specialk") || lower == "skif.exe"
        || lower.starts_with("easyanticheat") || lower.starts_with("beservice") || lower.starts_with("start_protected_game")
        || lower == "python.exe" || lower == "pythonw.exe" || lower.starts_with("python.") || lower.starts_with("pythonw.")
        || lower == "zsync.exe" || lower == "zsyncmake.exe"
        || lower == "vrwebhelper.exe" || lower.starts_with("cef")
        || lower == "7za.exe" || lower == "7z.exe" || lower.starts_with("7z")
        || lower == "scc.exe" || lower == "unins000.exe" || lower.starts_with("unins")
        || lower.contains("prelauncher") || lower.contains("redprelauncher")
        || lower.contains("errorreporter") || lower.contains("crashreporter")
        || lower == "vpk.exe" || lower.starts_with("vpk")
        || lower.contains("compiler") || lower.contains("compile")
        || lower == "hammer.exe" || lower == "vvis.exe" || lower == "vrad.exe" || lower == "vbsp.exe"
        || lower == "bspzip.exe" || lower == "glview.exe" || lower == "hlfaceposer.exe"
        || lower == "mksheet.exe" || lower == "motionmapper.exe" || lower == "qc_eyes.exe"
        || lower == "simd9.exe" || lower == "vtex.exe"
    {
        return true;
    }
    if lower.contains("installer") || lower.contains("uninstall") || lower.contains("crashreport") 
        || lower.contains("crashhandler") || lower.contains("vcredist") || lower.contains("dxsetup")
        || lower.contains("redist") || lower.contains("cleanup") || lower.contains("updater")
        || lower.contains("checker") || lower.contains("subprocess") || lower.contains("cefsharp")
        || lower.contains("browser") || lower.contains("crashmailer") || lower.contains("crashsender")
        || lower.contains("crash_report") || lower.contains("bugreport") || lower.contains("errorreport")
        || lower.contains("diagnostics") || lower.contains("benchmark")
        || lower.contains("prelauncher")
        || lower.contains("launcher")
        || lower.ends_with("config.exe") || lower.ends_with("_config.exe") || lower.ends_with("-config.exe") || lower == "config.exe" || lower.contains("configuration")
        || lower.ends_with("settings.exe") || lower.ends_with("_settings.exe") || lower.ends_with("-settings.exe") || lower == "settings.exe"
        || lower.ends_with("setup.exe") || lower.ends_with("_setup.exe") || lower.ends_with("-setup.exe") || lower == "setup.exe"
        || lower.contains("activation") || lower.starts_with("autorun") || lower == "autorun.exe"
        || lower.contains("registration") || lower == "register.exe"
        || lower.ends_with("support.exe") || lower.ends_with("_support.exe")
    {
        return true;
    }
    RE_INSTALLER.is_match(&lower)
}

pub fn is_helper_or_tool_path(path: &Path) -> bool {
    for comp in path.components() {
        let s = comp.as_os_str().to_string_lossy().to_lowercase();
        if s == "host64" || s == "optiscaler" || s == "_dlss5_backup" || s == "reshade-shaders"
            || s == "crashreporter" || s == "crashreports" || s == "tools" || s == "tool"
            || s == "compiler" || s == "compilers" || s == "sdk" || s == "sdks"
            || s == "easyanticheat" || s == "battleye" || s == "anticheat"
            || s == "__installer" || s == "installer_resources" || s == "installers" || s == "installer"
            || s == "support" || s == "redist" || s == "_redist" || s == "commonredist" || s == "_commonredist"
            || s == "prerequisites" || s == "directx"
            || s == "launcher" || s == "launchers" {
            return true;
        }
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    is_installer_or_helper(name)
}

pub fn is_not_a_game_dir(name: &str) -> bool {
    let lower = name.to_lowercase();
    match lower.as_str() {
        "steamapps" | "gamesave" | "gamesaves" | "workshop" | "downloading" 
        | "shadercache" | "cache" | "caches" | "temp" | "tmp" | "backup" 
        | "_dlss5_backup" | "reshade-shaders" | "host64" | "optiscaler" | "save" | "saves" | "savegame" 
        | "savegames" | "redist" | "commonredist" | "_commonredist" 
        | "__installer" | "installer" | "installers" | "setup" | "dlc" | "mods" | "mod" 
        | "tools" | "tool" | "node_modules" | ".git" | "sdk" | "patcher" => return true,
        _ => {}
    }
    if lower.contains("modmanager") || lower.contains("vortex") || lower.contains("fluffy") || lower.contains("trainer") {
        return true;
    }
    RE_NOT_GAME_DIR.is_match(&lower)
}

pub fn is_not_a_game_title(name: &str) -> bool {
    let lower = name.to_lowercase();
    RE_NOT_GAME_TITLE.is_match(&lower)
}

pub fn is_library_container_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    RE_CONTAINER.is_match(&lower)
}

pub fn holds_game<P: AsRef<Path>>(dir: P, depth: usize) -> bool {
    let dir = dir.as_ref();
    let Ok(entries) = fs::read_dir(dir) else { return false; };
    let mut subdirs = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                let lower = fname.to_lowercase();
                if lower.ends_with(".exe") && !is_helper_or_tool_path(&path) {
                    return true;
                }
            }
        } else if path.is_dir() && depth > 0 {
            if let Some(dname) = path.file_name().and_then(|n| n.to_str()) {
                if !is_not_a_game_dir(dname) {
                    subdirs.push(path);
                }
            }
        }
    }

    if depth == 0 {
        return false;
    }

    for sub in subdirs {
        if holds_game(&sub, depth - 1) {
            return true;
        }
    }

    false
}

pub fn api_from_names(imports: &[String]) -> Option<String> {
    let has = |n: &str| imports.iter().any(|i| i == n || i.ends_with(&format!("\\{}", n)) || i.ends_with(&format!("/{}", n)));
    if has("d3d12.dll") {
        return Some("DirectX 12".to_string());
    }
    if has("vulkan-1.dll") {
        return Some("Vulkan".to_string());
    }
    if has("d3d11.dll") {
        return Some("DirectX 11".to_string());
    }
    if has("d3d9.dll") {
        return Some("DirectX 9".to_string());
    }
    if has("d3d10.dll") || has("d3d10_1.dll") {
        return Some("DirectX 10".to_string());
    }
    if has("dxgi.dll") {
        return Some("DirectX (DXGI)".to_string());
    }
    if has("d3d8.dll") {
        return Some("DirectX 8".to_string());
    }
    None
}

pub fn api_from_markers(path: &Path) -> Option<String> {
    let markers = &[
        "D3D12CreateDevice", "D3D12SDKPath", "D3D12SDKVersion",
        "D3D11CreateDevice", "D3D10CreateDevice",
        "Direct3DCreate9", "Direct3DCreate9Ex", "Direct3DCreate8", "CreateDXGIFactory", "vkCreateInstance", "wglCreateContext"
    ];
    let found = find_markers(path, markers);
    if found.iter().any(|m| m == "D3D12CreateDevice" || m == "D3D12SDKPath" || m == "D3D12SDKVersion") {
        return Some("DirectX 12".to_string());
    }
    if found.iter().any(|m| m == "vkCreateInstance") {
        return Some("Vulkan".to_string());
    }
    if found.iter().any(|m| m == "D3D11CreateDevice") {
        return Some("DirectX 11".to_string());
    }
    if found.iter().any(|m| m == "Direct3DCreate9" || m == "Direct3DCreate9Ex") {
        return Some("DirectX 9".to_string());
    }
    if found.iter().any(|m| m == "D3D10CreateDevice") {
        return Some("DirectX 10".to_string());
    }
    if found.iter().any(|m| m == "CreateDXGIFactory") {
        return Some("DirectX (DXGI)".to_string());
    }
    if found.iter().any(|m| m == "Direct3DCreate8") {
        return Some("DirectX 8".to_string());
    }
    if found.iter().any(|m| m == "wglCreateContext") {
        return Some("OpenGL".to_string());
    }
    None
}

pub fn detect_renpy_api(dir: &Path) -> Option<String> {
    let is_renpy = dir.join("renpy").is_dir()
        || dir.join("lib").join("windows-x86_64").join("librenpython.dll").exists()
        || dir.join("lib").join("windows-i686").join("librenpython.dll").exists()
        || dir.join("librenpython.dll").exists();

    if !is_renpy {
        return None;
    }

    // Inspect log.txt in game directory if available
    let log_path = dir.join("log.txt");
    if let Ok(content) = fs::read_to_string(&log_path) {
        let c_lower = content.to_lowercase();
        if c_lower.contains("angle2") || c_lower.contains("directx 11") || c_lower.contains("d3d11") {
            return Some("DirectX 11".to_string());
        }
        if c_lower.contains("angle") || c_lower.contains("directx 9") || c_lower.contains("d3d9") {
            return Some("DirectX 9".to_string());
        }
    }

    // Ren'Py's default renderer on Windows is gl2 (Desktop OpenGL)
    Some("OpenGL".to_string())
}

fn is_middleware_dll(fname: &str) -> bool {
    let n_lower = fname.to_lowercase();
    if n_lower.contains("dx12") || n_lower.contains("d3d12") {
        return false;
    }
    n_lower.starts_with("sdl") || n_lower.starts_with("bink") || n_lower.starts_with("fmod") 
        || n_lower.starts_with("libxess") || n_lower.starts_with("nvngx") || n_lower.starts_with("amd_")
        || n_lower.starts_with("galaxy") || n_lower.starts_with("discord") || n_lower.starts_with("steam")
        || n_lower.starts_with("party") || n_lower.starts_with("playfab") || n_lower.starts_with("libhttpclient")
        || n_lower.starts_with("crash") || n_lower.starts_with("breakpad") || n_lower.starts_with("sentry")
        || n_lower.starts_with("bugsplat") || n_lower.starts_with("cef") || n_lower.starts_with("libcef")
        || n_lower.starts_with("ffmpeg") || n_lower.starts_with("avcodec") || n_lower.starts_with("avformat")
        || n_lower.starts_with("qt5") || n_lower.starts_with("qt6") || n_lower.starts_with("chrome_elf")
        || n_lower.starts_with("openimage") || n_lower.starts_with("tbb") || n_lower.starts_with("xcurl")
        || n_lower.starts_with("coherent") || n_lower.starts_with("physx") || n_lower.starts_with("apex")
        || n_lower.starts_with("eossdk") || n_lower.starts_with("libcurl") || n_lower.starts_with("msvcp")
        || n_lower.starts_with("vcruntime") || n_lower.starts_with("api-ms-") || n_lower.starts_with("ucrtbase")
}

pub fn detect_sibling_api(dir: &Path) -> Option<String> {
    if let Some(api) = detect_renpy_api(dir) {
        return Some(api);
    }
    if dir.join("D3D12Core.dll").exists() 
        || dir.join("D3D12").join("D3D12Core.dll").exists() 
        || dir.join("D3D12").is_dir() 
    {
        return Some("DirectX 12".to_string());
    }
    // DirectX 12 Shader Model 6 (DXC) compiler is exclusive to DirectX 12 and DXR ray tracing
    if dir.join("dxcompiler.dll").exists() || dir.join("dxil.dll").exists() {
        return Some("DirectX 12".to_string());
    }
    // Streamline DLSS-G / FSR Frame Generation DLLs are strictly DirectX 12
    if dir.join("sl.dlss_g.dll").exists() 
        || dir.join("amd_fidelityfx_framegeneration_dx12.dll").exists() 
    {
        return Some("DirectX 12".to_string());
    }

    let mut candidate_dlls: Vec<PathBuf> = Vec::new();
    let mut fallback_dxgi = false;

    // Scan dir itself plus standard game engine binary subdirectories
    let mut scan_dirs = vec![dir.to_path_buf()];
    let subdirs = ["bin", "bin64", "bin32", "x64", "x86", "win64", "win32", "retail", "Retail"];
    for sub in &subdirs {
        let p = dir.join(sub);
        if p.is_dir() && !scan_dirs.contains(&p) {
            scan_dirs.push(p);
        }
    }
    if let Some(parent) = dir.parent() {
        for sub in &subdirs {
            let p = parent.join(sub);
            if p.is_dir() && !scan_dirs.contains(&p) {
                scan_dirs.push(p);
            }
        }
    }

    for d in scan_dirs {
        let Ok(entries) = fs::read_dir(&d) else { continue; };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let is_dll = path.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case("dll")).unwrap_or(false);
                if !is_dll {
                    continue;
                }
                let fname = entry.file_name().to_string_lossy().to_lowercase();
                if is_middleware_dll(&fname) {
                    continue;
                }
                // Fast filename shortcuts
                if fname.contains("dx12") || fname.contains("d3d12") {
                    return Some("DirectX 12".to_string());
                }
                if fname.contains("vulkan") {
                    return Some("Vulkan".to_string());
                }
                if fname.contains("dx11") || fname.contains("d3d11") {
                    return Some("DirectX 11".to_string());
                }
                if fname.contains("dx9") || fname.contains("d3d9") || fname.contains("spdx9") || fname.contains("graphicsdx9") {
                    return Some("DirectX 9".to_string());
                }
                if fname.contains("dx8") || fname.contains("d3d8") {
                    return Some("DirectX 8".to_string());
                }
                if fname.contains("opengl") {
                    return Some("OpenGL".to_string());
                }

                candidate_dlls.push(path);
            }
        }
    }

    // Inspect PE imports & markers for candidate graphics DLLs
    // Prioritize DLLs likely to be rendering engines (d3d*, render*, gfx*, graphics*, shader*, engine*, etc.)
    candidate_dlls.sort_by_key(|p| {
        let fn_str = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        if fn_str.starts_with("d3d") || fn_str.starts_with("render") || fn_str.starts_with("gfx") || fn_str.starts_with("graphics") || fn_str.starts_with("shader") || fn_str.starts_with("engine") {
            0
        } else {
            1
        }
    });

    for path in candidate_dlls.iter().take(40) {
        if let Some(sib_pe) = inspect_pe(path) {
            if let Some(api) = api_from_names(&sib_pe.imports) {
                if api == "DirectX (DXGI)" {
                    fallback_dxgi = true;
                } else {
                    return Some(api);
                }
            }
            if let Some(api) = api_from_markers(path) {
                if api == "DirectX (DXGI)" {
                    fallback_dxgi = true;
                } else {
                    return Some(api);
                }
            }
        }
    }

    if fallback_dxgi {
        return Some("DirectX (DXGI)".to_string());
    }

    None
}

pub fn detect_api_for_exe(path: &Path) -> Option<String> {
    let imports = crate::core::pe::inspect_pe(path).map(|p| p.imports).unwrap_or_default();
    detect_api(path, &imports)
}

pub fn detect_api(path: &Path, imports: &[String]) -> Option<String> {
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if fname.contains("dx12") || fname.contains("d3d12") {
        return Some("DirectX 12".to_string());
    }
    if fname.contains("dx11") || fname.contains("d3d11") {
        return Some("DirectX 11".to_string());
    }
    if fname.contains("vulkan") {
        return Some("Vulkan".to_string());
    }

    if let Some(parent) = path.parent() {
        if let Some(api) = detect_renpy_api(parent) {
            return Some(api);
        }
    }
    if imports.iter().any(|i| i.to_lowercase().contains("librenpython")) {
        if let Some(parent) = path.parent() {
            if let Some(api) = detect_renpy_api(parent) {
                return Some(api);
            }
        }
        return Some("OpenGL".to_string());
    }

    let has = |n: &str| imports.iter().any(|i| i == n || i.ends_with(&format!("\\{}", n)) || i.ends_with(&format!("/{}", n)));

    // 1. Definite DirectX 12 indicators:
    // (a) Static import of d3d12.dll
    if has("d3d12.dll") {
        return Some("DirectX 12".to_string());
    }

    // (b) PE exports D3D12SDKVersion or D3D12SDKPath (Microsoft DirectX 12 Agility SDK)
    if let Some(pe) = inspect_pe(path) {
        if pe.exports.iter().any(|e| e == "D3D12SDKVersion" || e == "D3D12SDKPath") {
            return Some("DirectX 12".to_string());
        }
    }

    // (c) D3D12 markers in binary (D3D12CreateDevice / D3D12SDKPath / D3D12SDKVersion)
    let d3d12_markers = &["D3D12CreateDevice", "D3D12SDKPath", "D3D12SDKVersion"];
    let found_d3d12 = find_markers(path, d3d12_markers);
    if !found_d3d12.is_empty() {
        return Some("DirectX 12".to_string());
    }

    // (d) Directory contains DirectX 12 Agility SDK, DXC compiler, or Streamline / FSR Frame Generation DLLs
    if let Some(parent) = path.parent() {
        if parent.join("D3D12Core.dll").exists()
            || parent.join("D3D12").join("D3D12Core.dll").exists()
            || parent.join("D3D12").is_dir()
            || parent.join("dxcompiler.dll").exists()
            || parent.join("dxil.dll").exists()
            || parent.join("sl.dlss_g.dll").exists()
            || parent.join("amd_fidelityfx_framegeneration_dx12.dll").exists()
        {
            return Some("DirectX 12".to_string());
        }
    }

    // 2. Vulkan indicators:
    if has("vulkan-1.dll") {
        return Some("Vulkan".to_string());
    }
    let vk_found = find_markers(path, &["vkCreateInstance"]);
    if !vk_found.is_empty() {
        return Some("Vulkan".to_string());
    }

    // 3. DirectX 11 indicators (evaluated after DirectX 12 indicators):
    if has("d3d11.dll") {
        return Some("DirectX 11".to_string());
    }
    let d3d11_found = find_markers(path, &["D3D11CreateDevice"]);
    if !d3d11_found.is_empty() {
        return Some("DirectX 11".to_string());
    }

    // 4. Legacy DirectX & OpenGL:
    if has("d3d9.dll") {
        return Some("DirectX 9".to_string());
    }
    if has("d3d10.dll") || has("d3d10_1.dll") {
        return Some("DirectX 10".to_string());
    }
    if has("dxgi.dll") {
        return Some("DirectX (DXGI)".to_string());
    }
    if has("d3d8.dll") {
        return Some("DirectX 8".to_string());
    }
    if let Some(api) = api_from_markers(path) {
        return Some(api);
    }
    if imports.iter().any(|i| i == "opengl32.dll" || i.ends_with("\\opengl32.dll") || i.ends_with("/opengl32.dll")) {
        return Some("OpenGL".to_string());
    }

    // 5. Fallback: check imported sibling DLLs
    if let Some(parent) = path.parent() {
        let mut fallback_dxgi = false;
        for name in imports.iter().take(80) {
            if is_middleware_dll(name) {
                continue;
            }
            let sib_path = parent.join(name);
            if sib_path.is_file() {
                if let Some(sib_pe) = inspect_pe(&sib_path) {
                    if let Some(api) = api_from_names(&sib_pe.imports) {
                        if api == "DirectX (DXGI)" {
                            fallback_dxgi = true;
                        } else {
                            return Some(api);
                        }
                    }
                    if let Some(api) = api_from_markers(&sib_path) {
                        if api == "DirectX (DXGI)" {
                            fallback_dxgi = true;
                        } else {
                            return Some(api);
                        }
                    }
                }
            }
        }
        if fallback_dxgi {
            return Some("DirectX (DXGI)".to_string());
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct XboxExe {
    pub path: PathBuf,
    pub rel: String,
    pub name: String,
    pub bitness: u32,
}

pub fn xbox_executables(game_dir: &Path) -> Vec<XboxExe> {
    let mut configs = Vec::new();
    let root_cfg = game_dir.join("MicrosoftGame.config");
    if root_cfg.exists() {
        configs.push(root_cfg);
    }
    let content_cfg = game_dir.join("Content").join("MicrosoftGame.config");
    if content_cfg.exists() {
        configs.push(content_cfg);
    }

    let mut found = Vec::new();
    let exe_tag_re = Regex::new(r#"(?i)<Executable\b([^>]*)/?>"#).unwrap();
    let name_attr_re = Regex::new(r#"(?i)\bName\s*=\s*"([^"]+)""#).unwrap();
    let arch_attr_re = Regex::new(r#"(?i)\bArchitecture\s*=\s*"([^"]+)""#).unwrap();
    let proc_arch_re = Regex::new(r#"(?i)<ProcessorArchitecture>\s*([^<]+)\s*</ProcessorArchitecture>"#).unwrap();

    for config in configs {
        let Ok(text) = fs::read_to_string(&config) else { continue; };
        let cfg_dir = config.parent().unwrap_or(game_dir);

        let mut default_bitness = 64;
        if let Some(caps) = proc_arch_re.captures(&text) {
            let arch = caps.get(1).map(|m| m.as_str().trim().to_lowercase()).unwrap_or_default();
            if arch == "x86" {
                default_bitness = 32;
            }
        }

        for cap in exe_tag_re.captures_iter(&text) {
            let tag_str = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let name = if let Some(nc) = name_attr_re.captures(tag_str) {
                nc.get(1).map(|m| m.as_str().trim()).unwrap_or("")
            } else {
                continue;
            };

            let base_name = Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or(name);
            if base_name.eq_ignore_ascii_case("gamelaunchhelper.exe") || base_name.to_lowercase().starts_with("gamelaunchhelper") {
                continue;
            }

            let bitness = if let Some(ac) = arch_attr_re.captures(tag_str) {
                let arch = ac.get(1).map(|m| m.as_str().trim().to_lowercase()).unwrap_or_default();
                if arch == "x86" { 32 } else { 64 }
            } else {
                default_bitness
            };

            let full = cfg_dir.join(name.replace('/', "\\"));
            let rel = full.strip_prefix(game_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| base_name.to_string());

            found.push(XboxExe {
                path: full,
                rel,
                name: base_name.to_string(),
                bitness,
            });
        }
    }
    found
}

/// Discovers officially declared game executables from Xbox configs (MicrosoftGame.config, appxmanifest.xml)
/// and GOG manifests (goggame-*.info).
pub fn declared_executables(dir: &Path) -> (Vec<XboxExe>, Option<String>) {
    let mut found = xbox_executables(dir);
    let mut launcher = if !found.is_empty() {
        Some("Xbox".to_string())
    } else {
        None
    };

    // GOG manifests: goggame-*.info
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("goggame-") && name.ends_with(".info") {
                if launcher.is_none() {
                    launcher = Some("GOG".to_string());
                }
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(tasks) = val.get("playTasks").and_then(|v| v.as_array()) {
                            for task in tasks {
                                if let Some(cat) = task.get("category").and_then(|c| c.as_str()) {
                                    if cat != "game" {
                                        continue;
                                    }
                                }
                                if let Some(path_str) = task.get("path").and_then(|p| p.as_str()) {
                                    let full = dir.join(path_str.replace('/', "\\"));
                                    if full.is_file() {
                                        let file_name = full.file_name().unwrap_or_default().to_string_lossy().to_string();
                                        if !is_installer_or_helper(&file_name) && !found.iter().any(|x| x.path == full) {
                                            let pe_opt = inspect_pe(&full);
                                            let bitness = pe_opt.as_ref().map(|p| p.bitness).unwrap_or(64);
                                            let rel = full.strip_prefix(dir)
                                                .map(|p| p.to_string_lossy().to_string())
                                                .unwrap_or_else(|_| path_str.to_string());
                                            found.push(XboxExe {
                                                path: full,
                                                rel,
                                                name: file_name,
                                                bitness,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (found, launcher)
}

#[derive(Debug, Clone)]
struct Candidate {
    path: PathBuf,
    rel: String,
    name: String,
    size: u64,
    depth: usize,
    bitness: u32,
    is_laa: bool,
    api: String,
    declared: bool,
    has_sibling_dlss: bool,
    is_dx12: bool,
}

fn playable_role_score(name: &str, rel: &str) -> i64 {
    let lower = format!("{} {}", name, rel).to_lowercase();
    let mut score = 0;
    // Prioritize actual Unreal Engine gameplay shipping binaries over root bootstrap wrappers
    if lower.contains("shipping.exe") || lower.ends_with("-shipping.exe") || lower.ends_with("_shipping.exe") || lower.contains("win64-shipping") || lower.contains("wingdk-shipping") {
        score += 15000;
    }
    if lower.contains("singleplayer") || lower.ends_with("sp.exe") || lower.contains("_sp") {
        score += 400;
    }
    if lower.contains("multiplayer") || lower.ends_with("mp.exe") || lower.contains("_mp") {
        score -= 400;
    }
    // Boost Unreal Engine gameplay binaries (*game.exe)
    if lower.ends_with("game.exe") {
        score += 2000;
    }
    // Deprioritize legacy 32-bit fallback executables when 64-bit binaries exist
    if lower.contains("-32") || lower.contains("_32") || lower.contains("32bit") || lower.contains("win32") {
        score -= 2000;
    }
    score
}

fn candidate_score(c: &Candidate, dir_name: &str) -> i64 {
    let mut score = 0;
    if c.declared {
        score += 20000;
    }
    if c.has_sibling_dlss {
        score += 10000;
    }
    if c.is_dx12 {
        score += 5000;
    }
    if c.bitness == 64 {
        score += 1000;
    }
    // Prefer executables closer to root directory
    if c.depth <= 1 {
        score += 3000;
    }
    // Reward executables with verified graphics APIs
    let api_lower = c.api.to_lowercase();
    if api_lower.contains("directx") || api_lower.contains("vulkan") || api_lower.contains("opengl") {
        score += 5000;
    }
    // Boost significant game binary size (> 5MB)
    if c.size > 5_000_000 {
        score += 4000;
    }
    // Penalize small launcher stubs (< 1MB) that lack real graphics APIs
    if c.size < 1_000_000 && !c.declared && !c.has_sibling_dlss {
        score -= 5000;
    }
    // Boost executables matching the game folder name (e.g. BeingADIK.exe vs "Being a DIK")
    let norm_dir: String = dir_name.chars().filter(|ch| ch.is_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect();
    let norm_name: String = c.name.chars().filter(|ch| ch.is_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect();
    if !norm_dir.is_empty() && (norm_name.starts_with(&norm_dir) || norm_dir.starts_with(&norm_name)) {
        score += 8000;
    }
    score += playable_role_score(&c.name, &c.rel);
    score
}

pub fn extract_xbox_metadata(dir: &Path) -> (Option<String>, Option<String>) {
    let display_name_re = Regex::new(r#"(?i)DefaultDisplayName\s*=\s*"([^"]+)""#).unwrap();
    let display_name_tag_re = Regex::new(r#"(?i)<DisplayName>\s*([^<]+)\s*</DisplayName>"#).unwrap();
    let splash_re = Regex::new(r#"(?i)SplashScreenImage\s*=\s*"([^"]+)""#).unwrap();
    let logo_re = Regex::new(r#"(?i)Square150x150Logo\s*=\s*"([^"]+)""#).unwrap();
    let store_logo_re = Regex::new(r#"(?i)StoreLogo\s*=\s*"([^"]+)""#).unwrap();

    let mut resolved_name = None;
    let mut resolved_poster = None;

    let configs = [
        dir.join("MicrosoftGame.config"),
        dir.join("Content").join("MicrosoftGame.config"),
        dir.join("appxmanifest.xml"),
        dir.join("AppxManifest.xml"),
    ];

    for cfg in &configs {
        if let Ok(cfg_text) = fs::read_to_string(cfg) {
            if resolved_name.is_none() {
                if let Some(cap) = display_name_re.captures(&cfg_text).or_else(|| display_name_tag_re.captures(&cfg_text)) {
                    let name = cap[1].trim();
                    if !name.is_empty() && !name.starts_with("ms-resource:") {
                        resolved_name = Some(name.to_string());
                    }
                }
            }

            if resolved_poster.is_none() {
                if let Some(cap) = splash_re.captures(&cfg_text).or_else(|| logo_re.captures(&cfg_text)).or_else(|| store_logo_re.captures(&cfg_text)) {
                    let rel_img = cap[1].replace('/', "\\");
                    let img_path = dir.join(&rel_img);
                    if img_path.exists() {
                        resolved_poster = crate::core::steamart::file_to_art_uri(&img_path);
                    }
                }
            }

            if resolved_name.is_some() && resolved_poster.is_some() {
                break;
            }
        }
    }

    (resolved_name, resolved_poster)
}

pub fn find_local_gog_cover(game_id: &str) -> Option<PathBuf> {
    let mut base_dirs = Vec::new();
    if let Ok(prog_data) = std::env::var("ProgramData") {
        base_dirs.push(PathBuf::from(prog_data).join("GOG.com").join("Galaxy").join("webcache"));
    }
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        base_dirs.push(PathBuf::from(local_app).join("GOG.com").join("Galaxy").join("webcache"));
    }

    for base in base_dirs {
        if !base.is_dir() {
            continue;
        }
        if let Ok(users) = fs::read_dir(&base) {
            for user in users.flatten() {
                let gog_dir = user.path().join("gog").join(game_id);
                if gog_dir.is_dir() {
                    if let Ok(files) = fs::read_dir(&gog_dir) {
                        let mut fallbacks = Vec::new();
                        for f in files.flatten() {
                            let p = f.path();
                            let fname = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                            if fname.contains("_glx_vertical_cover") {
                                if let Ok(meta) = p.metadata() {
                                    if meta.len() > 2000 {
                                        return Some(p);
                                    }
                                }
                            } else if fname.contains("_glx_bg_") || fname.contains("_glx_logo") {
                                if let Ok(meta) = p.metadata() {
                                    if meta.len() > 2000 {
                                        fallbacks.push(p);
                                    }
                                }
                            }
                        }
                        if let Some(fb) = fallbacks.into_iter().next() {
                            return Some(fb);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn extract_gog_metadata(dir: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let mut resolved_name = None;
    let mut resolved_poster = None;
    let mut resolved_game_id = None;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_lowercase();
            if fname.starts_with("goggame-") && fname.ends_with(".info") {
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if resolved_name.is_none() {
                            if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
                                if !n.trim().is_empty() {
                                    resolved_name = Some(n.trim().to_string());
                                }
                            }
                        }
                        if resolved_game_id.is_none() {
                            if let Some(gid) = val.get("gameId").and_then(|v| v.as_str()) {
                                resolved_game_id = Some(gid.trim().to_string());
                            } else if let Some(gid_num) = val.get("gameId").and_then(|v| v.as_i64()) {
                                resolved_game_id = Some(gid_num.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    if resolved_game_id.is_none() || resolved_name.is_none() {
        let subkeys = win32_enum_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\GOG.com\Games", KEY_READ | KEY_WOW64_32KEY);
        let norm_dir = crate::core::state::normalize_game_path(dir);
        for game_id in subkeys {
            let subkey_path = format!(r"SOFTWARE\GOG.com\Games\{}", game_id);
            if let Some(path_str) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "path", KEY_READ | KEY_WOW64_32KEY) {
                if crate::core::state::normalize_path_str(&path_str) == norm_dir {
                    if resolved_game_id.is_none() {
                        resolved_game_id = Some(game_id.clone());
                    }
                    if resolved_name.is_none() {
                        if let Some(gname) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "gameName", KEY_READ | KEY_WOW64_32KEY) {
                            if !gname.trim().is_empty() {
                                resolved_name = Some(gname.trim().to_string());
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    if let Some(ref gid) = resolved_game_id {
        if let Some(cover_path) = find_local_gog_cover(gid) {
            resolved_poster = crate::core::steamart::file_to_art_uri(&cover_path);
        }
    }

    (resolved_name, resolved_poster, resolved_game_id)
}

pub fn is_generic_folder_name(s: &str) -> bool {
    let lower = s.trim().to_lowercase();
    matches!(
        lower.as_str(),
        "bin" | "x64" | "x86" | "win64" | "win32" | "binaries" | "retail" | "release" | "shipping" | "game" | "client"
    )
}

pub fn infer_game_name(dir: &Path, exe_path: &Path, xbox_name: Option<String>) -> String {
    if let Some(name) = xbox_name {
        if !name.trim().is_empty() {
            return name;
        }
    }

    let mut candidate_dir = dir;
    let mut resolved_name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();

    while is_generic_folder_name(&resolved_name) {
        if let Some(parent) = candidate_dir.parent() {
            if let Some(p_name) = parent.file_name() {
                resolved_name = p_name.to_string_lossy().to_string();
                candidate_dir = parent;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if is_generic_folder_name(&resolved_name) || resolved_name.trim().is_empty() {
        let exe_stem = exe_path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        if !exe_stem.is_empty() {
            return exe_stem;
        }
    }

    if resolved_name.trim().is_empty() {
        "Unknown Game".to_string()
    } else {
        resolved_name
    }
}

pub fn scan_game_directory<P: AsRef<Path>>(dir: P) -> Option<GameEntry> {
    let clean_dir = crate::core::state::clean_path_separators(dir.as_ref());
    let dir = clean_dir.as_path();
    if !dir.exists() || !dir.is_dir() {
        return None;
    }

    let mut dlss_files: Vec<(PathBuf, Option<String>)> = Vec::new();
    let mut has_fg = false;
    let mut mfg_addon = false;
    let mut addon_installed = false;
    let mut optiscaler_installed = false;
    let mut optiscaler_presr = false;
    let mut optiscaler_passes = 1;
    let mut poster = None;

    let (declared, declared_launcher) = declared_executables(dir);
    let mut candidates: Vec<Candidate> = Vec::new();

    let skip_dirs = [
        "_dlss5_backup", "reshade-shaders", "optiscaler", "host64", "paks", "movies", "saves", "logs",
        "node_modules", ".git", "data", "audio", "sound", "sounds", "music",
        "textures", "cinematics", "localization", "streamingassets", "shaders",
        "__installer", "installer_resources", "installers", "installer", "support", "redist", "_redist", "commonredist", "_commonredist", "prerequisites",
        "cache", "caches", "shadercache", "soundbanks", "soundbank", "video", "videos", "datas", "fonts", "font",
        "renpy", "crashreporter", "crashreports", "tools", "tool", "compiler", "compilers", "easyanticheat", "battleye",
        "launcher", "launchers"
    ];

    for entry in WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                !skip_dirs.contains(&name.as_str())
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let is_inside_mod_dir = path.components().any(|c| {
                let s = c.as_os_str().to_string_lossy().to_lowercase();
                s == "optiscaler" || s == "_dlss5_backup" || s == "reshade-shaders"
            });
            let file_name = entry.file_name().to_string_lossy().to_lowercase();
            if file_name.ends_with(".exe") && !is_helper_or_tool_path(path) && !is_installer_or_helper(&file_name) {
                let t_exe = std::time::Instant::now();
                let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                let depth = entry.depth();
                let is_declared = declared.iter().any(|x| x.path == path);
                let declared_match = declared.iter().find(|x| x.path == path);

                let pe_opt = inspect_pe(path);
                let bitness = if let Some(ref pe) = pe_opt {
                    pe.bitness
                } else if let Some(dm) = declared_match {
                    dm.bitness
                } else {
                    64
                };

                let parent_dir = path.parent();
                let has_sibling_dlss = parent_dir.map(|p| p.join("nvngx_dlss.dll").exists()).unwrap_or(false);

                let detected_api = pe_opt.as_ref()
                    .and_then(|pe| detect_api(path, &pe.imports))
                    .or_else(|| detect_api(path, &[]))
                    .or_else(|| parent_dir.and_then(detect_sibling_api));

                let api = match detected_api {
                    Some(a) => a,
                    None => {
                        if let Some(parent) = parent_dir {
                            if let Some(renpy_api) = detect_renpy_api(parent) {
                                renpy_api
                            } else if is_declared || depth <= 1 {
                                "Undetected".to_string()
                            } else {
                                println!("EXE REJECTED: {} took {:?}", path.display(), t_exe.elapsed());
                                continue;
                            }
                        } else if is_declared || depth <= 1 {
                            "Undetected".to_string()
                        } else {
                            println!("EXE REJECTED: {} took {:?}", path.display(), t_exe.elapsed());
                            continue;
                        }
                    }
                };
                println!("EXE ACCEPTED: {} took {:?}", path.display(), t_exe.elapsed());

                let is_dx12 = api == "DirectX 12";
                let rel = path.strip_prefix(dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());

                let is_laa = pe_opt.as_ref().map(|p| p.is_laa).unwrap_or(bitness == 64);
                candidates.push(Candidate {
                    path: path.to_path_buf(),
                    rel,
                    name: entry.file_name().to_string_lossy().to_string(),
                    size,
                    depth,
                    bitness,
                    is_laa,
                    api,
                    declared: is_declared,
                    has_sibling_dlss,
                    is_dx12,
                });
            } else if file_name.ends_with(".addon64") || file_name.ends_with(".addon32") || file_name.ends_with(".addon") {
                addon_installed = true;
                if file_name.contains("mfgunlock") {
                    mfg_addon = true;
                }
            } else if (file_name == "nvngx_dlss.dll" || file_name == "_nvngx.dll" || file_name == "nvngx.dll" || file_name == "nvngx_dlssnr.dll") && !is_inside_mod_dir {
                let pe_opt = inspect_pe(path);
                dlss_files.push((path.to_path_buf(), pe_opt.and_then(|p| p.version)));
            } else if (file_name == "nvngx_dlssg.dll" || file_name == "sl.dlss_g.dll" || file_name == "sl.dlss.dll" || file_name.contains("framegeneration_dx12") || file_name == "fgvk.dll") && !is_inside_mod_dir {
                has_fg = true;
                let pe_opt = inspect_pe(path);
                dlss_files.push((path.to_path_buf(), pe_opt.and_then(|p| p.version)));
            } else if file_name == "optiscaler.dll" || file_name == "optiscaler.ini" {
                optiscaler_installed = true;
                if file_name == "optiscaler.ini" {
                    if let Ok(ini_text) = fs::read_to_string(path) {
                        for line in ini_text.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("RunBeforeSR") && trimmed.contains("true") {
                                optiscaler_presr = true;
                            } else if trimmed.starts_with("PreSRMultipassCount") {
                                if let Some(val) = trimmed.split('=').nth(1) {
                                    if let Ok(p) = val.trim().parse::<u32>() {
                                        optiscaler_passes = p;
                                    }
                                }
                            }
                        }
                    }
                }
            } else if (file_name.contains("library_600x900") || file_name == "cover.jpg" || file_name == "poster.jpg") && poster.is_none() {
                poster = crate::core::steamart::file_to_art_uri(&path);
            }
        }
    }

    // Add manifest-declared executables that were not visited or had inspect_pe fail
    for d in &declared {
        if !d.path.exists() {
            continue;
        }
        if !candidates.iter().any(|c| c.path == d.path) {
            let size = fs::metadata(&d.path).map(|m| m.len()).unwrap_or(0);
            let parent_dir = d.path.parent();
            let has_sibling_dlss = parent_dir.map(|p| p.join("nvngx_dlss.dll").exists()).unwrap_or(false);
            let sibling_api = parent_dir.and_then(detect_sibling_api);
            let api = sibling_api.unwrap_or_else(|| "Undetected".to_string());
            let is_dx12 = api == "DirectX 12";

            let pe_d = crate::core::pe::inspect_pe(&d.path);
            let is_laa = pe_d.as_ref().map(|p| p.is_laa).unwrap_or(d.bitness == 64);

            candidates.push(Candidate {
                path: d.path.clone(),
                rel: d.rel.clone(),
                name: d.name.clone(),
                size,
                depth: d.rel.split(['/', '\\']).count().saturating_sub(1),
                bitness: d.bitness,
                is_laa,
                api,
                declared: true,
                has_sibling_dlss,
                is_dx12,
            });
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // If an Unreal Engine shipping binary exists in subdirectories (*-Shipping.exe),
    // filter out root-level dummy bootstrap wrappers (depth <= 1).
    let has_shipping_exe = candidates.iter().any(|c| {
        let l = c.name.to_lowercase();
        l.contains("shipping.exe") || l.ends_with("-shipping.exe") || l.ends_with("_shipping.exe")
    });
    if has_shipping_exe {
        candidates.retain(|c| {
            let l = c.name.to_lowercase();
            let is_shipping = l.contains("shipping.exe") || l.ends_with("-shipping.exe") || l.ends_with("_shipping.exe");
            is_shipping || c.depth > 1
        });
    }

    let dir_name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    candidates.sort_by(|a, b| {
        let a_score = candidate_score(a, &dir_name);
        let b_score = candidate_score(b, &dir_name);
        b_score.cmp(&a_score)
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.rel.cmp(&b.rel))
    });

    let available_exes: Vec<GameExeOption> = candidates.iter().map(|c| GameExeOption {
        name: c.name.clone(),
        path: c.path.clone(),
        rel: c.rel.clone(),
        api: c.api.clone(),
        bitness: c.bitness,
        is_laa: c.is_laa,
    }).collect();

    let manifest_opt = crate::core::journal::read_manifest(dir);
    let chosen = if let Some(m) = &manifest_opt {
        if let Some(target_exe_rel) = &m.game_exe {
            if let Some(pos) = candidates.iter().position(|c| c.rel.eq_ignore_ascii_case(target_exe_rel) || c.path.ends_with(target_exe_rel)) {
                candidates.remove(pos)
            } else {
                candidates.remove(0)
            }
        } else {
            candidates.remove(0)
        }
    } else {
        candidates.remove(0)
    };

    // Check ReShade hooks
    let mut reshade_installed = false;
    let mut reshade_version = None;
    let mut reshade_addon_support = false;

    let search_dirs = [chosen.path.parent(), Some(dir)];
    let hook_names = ["dxgi.dll", "d3d12.dll", "d3d11.dll", "d3d9.dll", "opengl32.dll", "dinput8.dll"];
    for s_dir in search_dirs.into_iter().flatten() {
        for hook in &hook_names {
            let hook_path = s_dir.join(hook);
            if hook_path.is_file() {
                let (mentions, ver, addon_sup) = crate::core::pe::is_reshade_dll(&hook_path);
                if mentions {
                    reshade_installed = true;
                    reshade_version = ver;
                    reshade_addon_support = addon_sup;
                    break;
                }
            }
        }
        if reshade_installed {
            break;
        }
    }

    // Check NRStyle from ReShade.ini, host64/ReShade.ini, or OptiScaler.ini
    let mut nr_style: usize = 0;
    for s_dir in search_dirs.into_iter().flatten() {
        let reshade_ini = s_dir.join("ReShade.ini");
        if reshade_ini.is_file() {
            if let Ok(text) = fs::read_to_string(&reshade_ini) {
                if let Some(val) = crate::core::optiscaler::get_ini(&text, "RenoDX.DLSS5", "NRStyle") {
                    if let Ok(parsed) = val.trim().parse::<usize>() {
                        nr_style = parsed;
                        break;
                    }
                }
            }
        }
        let host_reshade = s_dir.join("host64").join("ReShade.ini");
        if host_reshade.is_file() {
            if let Ok(text) = fs::read_to_string(&host_reshade) {
                if let Some(val) = crate::core::optiscaler::get_ini(&text, "RenoDX.DLSS5", "NRStyle") {
                    if let Ok(parsed) = val.trim().parse::<usize>() {
                        nr_style = parsed;
                        break;
                    }
                }
            }
        }
        let opti_ini = s_dir.join("OptiScaler.ini");
        if opti_ini.is_file() {
            if let Ok(text) = fs::read_to_string(&opti_ini) {
                if let Some(val) = crate::core::optiscaler::get_ini(&text, "DlssNr", "Style") {
                    if let Ok(parsed) = val.trim().parse::<usize>() {
                        nr_style = parsed;
                        break;
                    }
                }
            }
        }
    }

    // Check backup manifest
    let has_backup = crate::core::journal::has_backup_available(dir);

    let mut installed_route = None;
    let mut mfg_multiplier: u32 = 4;
    let mut nr_style_enabled = nr_style > 0;

    if let Some(manifest) = &manifest_opt {
        if !manifest.route.is_empty() {
            installed_route = Some(manifest.route.clone());
            if manifest.route == "feeder" || manifest.route == "native" {
                addon_installed = true;
                reshade_installed = true;
            } else if manifest.route == "optiscaler" {
                optiscaler_installed = true;
            }
        }
        if let Some(mfg_u) = manifest.mfg_unlock {
            mfg_addon = mfg_u;
        }
        if let Some(mult) = manifest.mfg_multiplier {
            mfg_multiplier = mult;
        }
        if let Some(nr_en) = manifest.nr_style_enabled {
            nr_style_enabled = nr_en;
        }
        if let Some(style) = manifest.nr_style {
            nr_style = style;
        }
        if let Some(presr) = manifest.opti_presr {
            optiscaler_presr = presr;
        }
        if let Some(passes) = manifest.opti_passes {
            optiscaler_passes = passes;
        }
    }

    // Disk fallback if manifest route was missing/empty:
    if installed_route.is_none() {
        let has_feeder = search_dirs.into_iter().flatten().any(|d| {
            d.join("dlss5-feed.addon64").is_file()
                || d.join("dlss5-feed.addon32").is_file()
                || d.join("dlss5-feed.cfg").is_file()
                || d.join("reshade-shaders").is_dir()
        });
        let has_native = search_dirs.into_iter().flatten().any(|d| {
            d.join("renodx-dlss5.addon64").is_file()
                || d.join("dlss5-d3d12-fix.addon64").is_file()
        });
        if has_feeder {
            installed_route = Some("feeder".to_string());
            addon_installed = true;
            reshade_installed = true;
        } else if has_native {
            installed_route = Some("native".to_string());
            addon_installed = true;
            reshade_installed = true;
        } else if optiscaler_installed {
            installed_route = Some("optiscaler".to_string());
        }
    }

    // GDK / MicrosoftGame.config fallback:
    // If Xbox declared and api is DirectX 12 without explicit D3D12 SDK, Streamline FG, or D3D12 binary proof, label as DirectX 11/12
    let api = if chosen.declared && chosen.api == "DirectX 12" {
        let has_d3d12_sdk_or_fg = chosen.path.parent().map(|p| {
            p.join("D3D12Core.dll").exists() 
                || p.join("D3D12").join("D3D12Core.dll").exists()
                || p.join("D3D12").is_dir()
                || p.join("dxcompiler.dll").exists()
                || p.join("dxil.dll").exists()
                || p.join("sl.dlss_g.dll").exists()
                || p.join("sl.interposer.dll").exists()
                || p.join("amd_fidelityfx_framegeneration_dx12.dll").exists()
        }).unwrap_or(false);

        let has_d3d12_binary_proof = if has_d3d12_sdk_or_fg || has_fg {
            true
        } else if let Some(pe) = inspect_pe(&chosen.path) {
            pe.imports.iter().any(|i| i.eq_ignore_ascii_case("d3d12.dll"))
                || pe.exports.iter().any(|e| e == "D3D12SDKVersion" || e == "D3D12SDKPath")
        } else {
            false
        };

        if !has_d3d12_sdk_or_fg && !has_d3d12_binary_proof {
            "DirectX 11/12".to_string()
        } else {
            chosen.api.clone()
        }
    } else {
        chosen.api.clone()
    };

    let chosen_dir = chosen.path.parent();
    dlss_files.sort_by(|a, b| {
        let a_same_dir = chosen_dir.map(|p| p == a.0.parent().unwrap_or(p)).unwrap_or(false);
        let b_same_dir = chosen_dir.map(|p| p == b.0.parent().unwrap_or(p)).unwrap_or(false);
        let a_is_primary = a.0.file_name().map(|n| n.to_string_lossy().to_lowercase() == "nvngx_dlss.dll").unwrap_or(false);
        let b_is_primary = b.0.file_name().map(|n| n.to_string_lossy().to_lowercase() == "nvngx_dlss.dll").unwrap_or(false);
        (b_same_dir as u8).cmp(&(a_same_dir as u8))
            .then_with(|| (b_is_primary as u8).cmp(&(a_is_primary as u8)))
            .then_with(|| (b.1.is_some() as u8).cmp(&(a.1.is_some() as u8)))
    });
    let sr_file = dlss_files.iter().find(|(p, _)| {
        let n = p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        n == "nvngx_dlss.dll" || n == "_nvngx.dll" || n == "nvngx.dll"
    });
    let dlss_version = sr_file.and_then(|f| f.1.clone());

    let mut files: Vec<GameFileItem> = Vec::new();
    for (f_path, ver) in &dlss_files {
        let rel = f_path.strip_prefix(dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| f_path.file_name().unwrap_or_default().to_string_lossy().to_string());
        if !files.iter().any(|existing| existing.rel == rel) {
            files.push(GameFileItem { rel, version: ver.clone() });
        }
    }
    files.sort_by(|a, b| a.rel.cmp(&b.rel));

    if poster.is_none() {
        poster = crate::core::steamart::find_cached_art(dir);
    }

    let (xbox_name, xbox_poster) = extract_xbox_metadata(dir);
    if poster.is_none() {
        poster = xbox_poster;
    }

    let (gog_name, gog_poster, gog_id) = extract_gog_metadata(dir);
    if poster.is_none() {
        poster = gog_poster;
    }

    let is_xbox_dir = dir.join("MicrosoftGame.config").exists()
        || dir.join("Content").join("MicrosoftGame.config").exists()
        || dir.join("AppxManifest.xml").exists()
        || dir.join("appxmanifest.xml").exists()
        || dir.to_string_lossy().to_lowercase().contains("xboxgames")
        || dir.to_string_lossy().to_lowercase().contains("windowsapps");
    let name = if let Some(g_name) = gog_name {
        g_name
    } else {
        infer_game_name(dir, &chosen.path, xbox_name)
    };

    crate::core::logger::debug("scan", &format!(
        "Game scanned '{}': exe={}, bitness={}-bit, api={}, dlss={:?}, fg={}, optiscaler={}, backup={}",
        name, chosen.rel, chosen.bitness, api, dlss_version, has_fg, optiscaler_installed, has_backup
    ));

    let detected_launcher = if let Some(dl) = declared_launcher {
        dl
    } else if gog_id.is_some() {
        "GOG".to_string()
    } else if is_xbox_dir {
        "Xbox".to_string()
    } else if dir.join("steam_appid.txt").exists()
        || dir.to_string_lossy().to_lowercase().contains("steamapps")
    {
        "Steam".to_string()
    } else if dir.join(".egstore").exists()
        || dir.to_string_lossy().to_lowercase().contains("epic games")
    {
        "Epic Games".to_string()
    } else if fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                e.file_name().to_string_lossy().to_lowercase().starts_with("goggame-")
            })
        })
        .unwrap_or(false)
        || dir.to_string_lossy().to_lowercase().contains("gog")
    {
        "GOG".to_string()
    } else {
        "Added by hand".to_string()
    };

    let is_mod_added_dlss = if let Some(manifest) = crate::core::journal::read_manifest(dir) {
        if let Some((sr_p, _)) = &sr_file {
            let rel = sr_p.strip_prefix(dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| sr_p.file_name().unwrap_or_default().to_string_lossy().to_string());
            manifest.added.iter().any(|a| a.eq_ignore_ascii_case(&rel))
        } else {
            false
        }
    } else {
        false
    };
    let has_native_dlss = sr_file.is_some() && !is_mod_added_dlss;
    let api_lower = chosen.api.to_lowercase();
    let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");
    let is_vulkan = api_lower.contains("vulkan");
    let can_inject_fg = chosen.bitness == 64 && has_native_dlss && !has_fg && (is_dx12 || is_vulkan);
    let has_anti_cheat = crate::core::install_guards::has_anti_cheat(dir);

    Some(GameEntry {
        name,
        dir: dir.to_path_buf(),
        exe_path: chosen.path,
        exe_rel: chosen.rel,
        bitness: chosen.bitness,
        is_laa: chosen.is_laa,
        api,
        dlss_version,
        has_frame_generation: has_fg,
        can_inject_fg,
        optiscaler_installed,
        optiscaler_presr,
        optiscaler_passes,
        mfg_unlock_installed: mfg_addon,
        has_backup,
        launcher: detected_launcher,
        poster,
        reshade_installed,
        reshade_version,
        reshade_addon_support,
        addon_installed,
        installed_route,
        files,
        available_exes,
        nr_style,
        nr_style_enabled,
        mfg_multiplier,
        has_anti_cheat,
    })
}

/// Helper to discover all valid game executables in a game directory on demand.
pub fn discover_game_exes(dir: &Path) -> Vec<GameExeOption> {
    let mut exes = Vec::new();
    let (declared, _) = declared_executables(dir);
    for d in &declared {
        let pe_opt = inspect_pe(&d.path);
        let bitness = pe_opt.as_ref().map(|p| p.bitness).unwrap_or(d.bitness);
        let detected_api = pe_opt.as_ref()
            .and_then(|pe| detect_api(&d.path, &pe.imports))
            .or_else(|| detect_api(&d.path, &[]))
            .or_else(|| d.path.parent().and_then(detect_sibling_api));
        let is_laa = pe_opt.as_ref().map(|p| p.is_laa).unwrap_or(bitness == 64);
        let api = detected_api.unwrap_or_else(|| "Undetected".to_string());
        exes.push(GameExeOption {
            name: d.name.clone(),
            path: d.path.clone(),
            rel: d.rel.clone(),
            api,
            bitness,
            is_laa,
        });
    }

    let skip_dirs = [
        "_dlss5_backup", "reshade-shaders", "optiscaler", "host64", "paks", "movies", "saves", "logs",
        "node_modules", ".git", "data", "audio", "sound", "sounds", "music",
        "textures", "cinematics", "localization", "streamingassets", "shaders",
        "__installer", "installer_resources", "installers", "installer", "support", "redist", "_redist", "commonredist", "_commonredist", "prerequisites",
        "cache", "caches", "shadercache", "soundbanks", "soundbank", "video", "videos", "datas", "fonts", "font",
        "renpy", "crashreporter", "crashreports", "tools", "tool", "compiler", "compilers", "easyanticheat", "battleye",
        "launcher", "launchers"
    ];

    for entry in WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                !skip_dirs.contains(&name.as_str())
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let file_name = entry.file_name().to_string_lossy().to_lowercase();
            if file_name.ends_with(".exe") && !is_helper_or_tool_path(path) && !is_installer_or_helper(&file_name) {
                if exes.iter().any(|e| e.path == path) {
                    continue;
                }
                let pe_opt = inspect_pe(path);
                let bitness = pe_opt.as_ref().map(|p| p.bitness).unwrap_or(64);
                let is_laa = pe_opt.as_ref().map(|p| p.is_laa).unwrap_or(bitness == 64);
                let detected_api = pe_opt.as_ref()
                    .and_then(|pe| detect_api(path, &pe.imports))
                    .or_else(|| detect_api(path, &[]))
                    .or_else(|| path.parent().and_then(detect_sibling_api));

                let api = detected_api.unwrap_or_else(|| "Undetected".to_string());
                let rel = path.strip_prefix(dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());
                exes.push(GameExeOption {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: path.to_path_buf(),
                    rel,
                    api,
                    bitness,
                    is_laa,
                });
            }
        }
    }

    // If an Unreal Engine shipping binary exists in subdirectories (*-Shipping.exe),
    // filter out root-level dummy bootstrap wrappers (depth <= 1).
    let has_shipping_exe = exes.iter().any(|e| {
        let l = e.name.to_lowercase();
        l.contains("shipping.exe") || l.ends_with("-shipping.exe") || l.ends_with("_shipping.exe")
    });
    if has_shipping_exe {
        exes.retain(|e| {
            let l = e.name.to_lowercase();
            let is_shipping = l.contains("shipping.exe") || l.ends_with("-shipping.exe") || l.ends_with("_shipping.exe");
            let depth = e.rel.matches(['/', '\\']).count();
            is_shipping || depth > 1
        });
    }

    let dir_name: String = dir.file_name().unwrap_or_default().to_string_lossy().chars().filter(|ch| ch.is_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect();
    exes.sort_by(|a, b| {
        let a_is_shipping = a.name.to_lowercase().contains("shipping.exe");
        let b_is_shipping = b.name.to_lowercase().contains("shipping.exe");
        let a_decl_pos = declared.iter().position(|d| d.path == a.path).unwrap_or(usize::MAX);
        let b_decl_pos = declared.iter().position(|d| d.path == b.path).unwrap_or(usize::MAX);
        let a_is_32 = a.name.to_lowercase().contains("-32") || a.name.to_lowercase().contains("_32") || a.name.to_lowercase().contains("32bit");
        let b_is_32 = b.name.to_lowercase().contains("-32") || b.name.to_lowercase().contains("_32") || b.name.to_lowercase().contains("32bit");
        let a_norm: String = a.name.chars().filter(|ch| ch.is_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect();
        let b_norm: String = b.name.chars().filter(|ch| ch.is_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect();
        let a_match = !dir_name.is_empty() && (a_norm.starts_with(&dir_name) || dir_name.starts_with(&a_norm));
        let b_match = !dir_name.is_empty() && (b_norm.starts_with(&dir_name) || dir_name.starts_with(&b_norm));

        (b_is_shipping as u8).cmp(&(a_is_shipping as u8))
            .then_with(|| a_decl_pos.cmp(&b_decl_pos))
            .then_with(|| (a_is_32 as u8).cmp(&(b_is_32 as u8)))
            .then_with(|| (b_match as u8).cmp(&(a_match as u8)))
            .then_with(|| a.rel.matches(['/', '\\']).count().cmp(&b.rel.matches(['/', '\\']).count()))
            .then_with(|| b.bitness.cmp(&a.bitness))
            .then_with(|| a.name.cmp(&b.name))
    });
    exes
}

pub fn scan_library_root<P: AsRef<Path>>(root: P) -> Vec<GameEntry> {
    let root = root.as_ref();
    let mut games = Vec::new();
    if !root.exists() || !root.is_dir() {
        return games;
    }

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name_str) = path.file_name().and_then(|n| n.to_str()) {
                    if is_not_a_game_dir(name_str) || is_not_a_game_title(name_str) {
                        continue;
                    }
                    if is_library_container_name(name_str) {
                        games.extend(scan_library_root(&path));
                        continue;
                    }
                    let t_folder = std::time::Instant::now();
                    let holds = holds_game(&path, 3);
                    println!("FOLDER: {} => holds={}, took={:?}", path.display(), holds, t_folder.elapsed());
                    if holds {
                        let t_scan = std::time::Instant::now();
                        if let Some(mut game) = scan_game_directory(&path) {
                            println!("SCANNED GAME: {} => took={:?}", game.name, t_scan.elapsed());
                            game.launcher = "My folders".to_string();
                            games.push(game);
                        }
                    }
                }
            }
        }
    }
    games
}

pub fn dedupe_games(games: Vec<GameEntry>) -> Vec<GameEntry> {
    let mut map: std::collections::HashMap<String, GameEntry> = std::collections::HashMap::new();
    for game in games {
        let key = game.dir.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
        if let Some(existing) = map.get(&key) {
            let existing_is_generic = existing.launcher.starts_with("My folders") || existing.launcher == "Added by hand";
            let new_is_launcher = !game.launcher.starts_with("My folders") && game.launcher != "Added by hand";
            if existing_is_generic && new_is_launcher {
                map.insert(key, game);
            }
        } else {
            map.insert(key, game);
        }
    }
    let mut list: Vec<GameEntry> = map.into_values().collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

pub fn discover_steam() -> Vec<GameEntry> {
    let mut games = Vec::new();
    #[cfg(windows)]
    let steam_path = win32_read_reg_string(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath", KEY_READ);
    #[cfg(not(windows))]
    let steam_path: Option<String> = None;

    let Some(steam_root) = steam_path else {
        return games;
    };

    let mut libraries = Vec::new();
    let mut seen_libs = std::collections::HashSet::new();

    let vdf_path = PathBuf::from(&steam_root).join("steamapps").join("libraryfolders.vdf");
    if let Ok(vdf_content) = fs::read_to_string(&vdf_path) {
        let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
        for cap in re.captures_iter(&vdf_content) {
            let lib = cap[1].replace(r"\\", r"\");
            let clean_lib = crate::core::state::clean_path_separators(Path::new(&lib));
            let norm_lib = crate::core::state::normalize_game_path(&clean_lib);
            if seen_libs.insert(norm_lib) {
                libraries.push(clean_lib);
            }
        }
    }

    // Fallback if libraryfolders.vdf was missing or did not include steam_root
    let clean_root = crate::core::state::clean_path_separators(Path::new(&steam_root));
    let norm_root = crate::core::state::normalize_game_path(&clean_root);
    if seen_libs.insert(norm_root) {
        libraries.push(clean_root);
    }

    let mut seen_dirs = std::collections::HashSet::new();
    for lib in libraries {
        let apps_dir = lib.join("steamapps");
        let Ok(entries) = fs::read_dir(&apps_dir) else { continue; };
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with("appmanifest_") && fname.ends_with(".acf") {
                if let Ok(acf_text) = fs::read_to_string(entry.path()) {
                    let appid = Regex::new(r#""appid"\s+"([^"]+)""#).ok().and_then(|r| r.captures(&acf_text).map(|c| c[1].to_string()));
                    let installdir = Regex::new(r#""installdir"\s+"([^"]+)""#).ok().and_then(|r| r.captures(&acf_text).map(|c| c[1].to_string()));
                    let gname = Regex::new(r#""name"\s+"([^"]+)""#).ok().and_then(|r| r.captures(&acf_text).map(|c| c[1].to_string()));

                    if let (Some(aid), Some(idir)) = (appid, installdir) {
                        if let Some(ref title) = gname {
                            if is_not_a_game_title(title) {
                                continue;
                            }
                        }
                        let game_dir = apps_dir.join("common").join(&idir);
                        let norm = crate::core::state::normalize_game_path(&game_dir);
                        if !seen_dirs.insert(norm) {
                            continue;
                        }
                        if game_dir.exists() {
                            if let Some(mut game) = scan_game_directory(&game_dir) {
                                game.launcher = "Steam".to_string();
                                if let Some(real_name) = gname {
                                    game.name = real_name;
                                }
                                let poster_path = PathBuf::from(&steam_root).join("appcache").join("librarycache").join(&aid).join("library_600x900.jpg");
                                if poster_path.exists() {
                                    game.poster = crate::core::steamart::file_to_art_uri(&poster_path);
                                }
                                games.push(game);
                            }
                        }
                    }
                }
            }
        }
    }
    games
}

pub fn discover_gog() -> Vec<GameEntry> {
    let mut games = Vec::new();
    #[cfg(windows)]
    {
        let subkeys = win32_enum_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\GOG.com\Games", KEY_READ | KEY_WOW64_32KEY);
        let mut seen_paths = std::collections::HashSet::new();
        for game_id in subkeys {
            let subkey_path = format!(r"SOFTWARE\GOG.com\Games\{}", game_id);
            if let Some(path_str) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "path", KEY_READ | KEY_WOW64_32KEY) {
                let norm = crate::core::state::normalize_path_str(&path_str);
                if !seen_paths.insert(norm) {
                    continue;
                }
                let gdir = PathBuf::from(path_str.trim());
                if gdir.exists() {
                    if let Some(mut game) = scan_game_directory(&gdir) {
                        game.launcher = "GOG".to_string();
                        if let Some(gname) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &subkey_path, "gameName", KEY_READ | KEY_WOW64_32KEY) {
                            if !gname.trim().is_empty() {
                                game.name = gname.trim().to_string();
                            }
                        }
                        games.push(game);
                    }
                }
            }
        }
    }
    games
}

pub fn discover_epic() -> Vec<GameEntry> {
    let mut games = Vec::new();
    let manifests = PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests");
    let mut seen_paths = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir(&manifests) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "item").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(loc) = val.get("InstallLocation").and_then(|v| v.as_str()) {
                            let norm = crate::core::state::normalize_path_str(loc);
                            if !seen_paths.insert(norm) {
                                continue;
                            }
                            let gdir = PathBuf::from(loc);
                            if gdir.exists() {
                                if let Some(mut game) = scan_game_directory(&gdir) {
                                    game.launcher = "Epic Games".to_string();
                                    if let Some(dname) = val.get("DisplayName").and_then(|v| v.as_str()) {
                                        game.name = dname.to_string();
                                    }
                                    games.push(game);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    games
}

pub fn discover_xbox() -> Vec<GameEntry> {
    let mut games = Vec::new();
    let mut candidate_dirs: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        // 1. Query HKLM\SOFTWARE\Microsoft\GamingServices\PackageRepository\Root
        let root_subs = win32_enum_subkeys(HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\GamingServices\PackageRepository\Root", KEY_READ);
        for sub in root_subs {
            let sub_path = format!(r"SOFTWARE\Microsoft\GamingServices\PackageRepository\Root\{}", sub);
            let child_subs = win32_enum_subkeys(HKEY_LOCAL_MACHINE, &sub_path, KEY_READ);
            for child in child_subs {
                let pkg_key = format!(r"{}\{}", sub_path, child);
                if let Some(raw_root) = win32_read_reg_string(HKEY_LOCAL_MACHINE, &pkg_key, "Root", KEY_READ) {
                    let clean = raw_root.trim_start_matches(r"\\?\").trim_end_matches('\\').trim_end_matches('/');
                    let p = PathBuf::from(clean);
                    if p.exists() && p.is_dir() && !candidate_dirs.contains(&p) {
                        candidate_dirs.push(p);
                    }
                }
            }
        }
    }

    // 2. Scan standard XboxGames folders on all fixed drives
    for drive in get_fixed_drives() {
        let xgames = drive.join("XboxGames");
        if xgames.is_dir() {
            if let Ok(entries) = fs::read_dir(&xgames) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && !candidate_dirs.contains(&p) {
                        candidate_dirs.push(p);
                    }
                }
            }
        }
    }

    // 3. Scan candidate directories
    for dir in candidate_dirs {
        if let Some(mut game) = scan_game_directory(&dir) {
            game.launcher = "Xbox".to_string();

            let (xbox_name, xbox_poster) = extract_xbox_metadata(&dir);
            if let Some(name) = xbox_name {
                game.name = name;
            }
            if game.poster.is_none() {
                game.poster = xbox_poster;
            }

            games.push(game);
        }
    }

    games
}

pub fn discover_all_launchers() -> Vec<GameEntry> {
    crate::core::logger::info("scan", "Starting discovery across all launchers (Steam, GOG, Epic, Xbox)...");
    let t0 = std::time::Instant::now();
    let mut all = Vec::new();

    let steam_games = discover_steam();
    crate::core::logger::info("scan", &format!("Steam discovery completed: {} games found", steam_games.len()));
    all.extend(steam_games);

    let gog_games = discover_gog();
    crate::core::logger::info("scan", &format!("GOG discovery completed: {} games found", gog_games.len()));
    all.extend(gog_games);

    let epic_games = discover_epic();
    crate::core::logger::info("scan", &format!("Epic Games discovery completed: {} games found", epic_games.len()));
    all.extend(epic_games);

    let xbox_games = discover_xbox();
    crate::core::logger::info("scan", &format!("Xbox Game Pass discovery completed: {} games found", xbox_games.len()));
    all.extend(xbox_games);

    crate::core::logger::info("scan", &format!("All launchers discovery finished in {:?}: {} total games found", t0.elapsed(), all.len()));
    all
}

pub fn get_fixed_drives() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLogicalDrives() -> u32;
            fn GetDriveTypeW(lpRootPathName: *const u16) -> u32;
        }

        const DRIVE_FIXED: u32 = 3;
        let mut drives = Vec::new();
        let mask = unsafe { GetLogicalDrives() };

        for i in 2..26 { // Start at C (index 2: 'C' - 'A')
            if (mask & (1 << i)) != 0 {
                let letter = (b'A' + i as u8) as char;
                let root_str = format!("{}:\\", letter);
                let wide: Vec<u16> = root_str.encode_utf16().chain(std::iter::once(0)).collect();
                let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
                if drive_type == DRIVE_FIXED {
                    drives.push(PathBuf::from(root_str));
                }
            }
        }
        drives
    }

    #[cfg(not(windows))]
    {
        vec![PathBuf::from("/")]
    }
}

pub fn is_inside<P1: AsRef<Path>, P2: AsRef<Path>>(file: P1, root: P2) -> bool {
    let candidate = file.as_ref().to_string_lossy().to_lowercase().replace('/', "\\");
    let parent = root.as_ref().to_string_lossy().to_lowercase().replace('/', "\\");
    candidate == parent || candidate.starts_with(&format!("{}\\", parent.trim_end_matches('\\')))
}

const SYSTEM_DIRS: &[&str] = &[
    "windows", "winnt", "program files", "program files (x86)", "programdata",
    "users", "$recycle.bin", "system volume information", "recovery", "perflogs",
    "config.msi", "documents and settings", "msocache", "intel", "amd", "nvidia",
    "drivers", "temp", "tmp", "$windows.~bt", "$windows.~ws", "onedrivetemp",
    "inetpub", "node_modules"
];

pub fn discover_drive_roots(excluded_roots: &[String]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let library_name_re = Regex::new(r"(?i)^(games?|my ?games|steamlibrary|gog ?games|epic ?games|xbox ?games|origin ?games|repacks?|emulation)$").unwrap();

    for drive in get_fixed_drives() {
        let Ok(entries) = fs::read_dir(&drive) else { continue; };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() { continue; }
            let Some(name_str) = path.file_name().and_then(|n| n.to_str()) else { continue; };
            let name_lower = name_str.to_lowercase();
            if SYSTEM_DIRS.contains(&name_lower.as_str()) {
                continue;
            }

            if library_name_re.is_match(&name_lower) {
                if !excluded_roots.iter().any(|ex| is_inside(&path, ex)) {
                    roots.push(path);
                }
                continue;
            }

            if let Ok(kids) = fs::read_dir(&path) {
                let kid_dirs: Vec<PathBuf> = kids.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
                if kid_dirs.len() >= 2 && kid_dirs.len() <= 300 {
                    let gameish = kid_dirs.iter().filter(|k| holds_game(k, 2)).count();
                    if gameish >= 2 && gameish * 2 >= kid_dirs.len() {
                        if !excluded_roots.iter().any(|ex| is_inside(&path, ex)) {
                            roots.push(path);
                        }
                    }
                }
            }
        }
    }
    roots
}

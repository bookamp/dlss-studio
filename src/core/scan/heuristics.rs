#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use regex::Regex;
use walkdir::WalkDir;
use crate::core::pe::{inspect_pe, find_markers};
use super::{GameExeOption, xbox::declared_executables};

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
    Regex::new(r"(?i)(redistributabl|steamworks common|steamworks shared|directx|vcredist|proton|steam linux runtime|soundtrack|modmanager|mod manager|save editor|trainer|cheat engine|nexus mods|sdk )").unwrap()
});

static RE_CONTAINER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(games?|my ?games|steamlibrary|gog ?games|gog|epic ?games|epic|xbox ?games|origin ?games|repacks?|emulation)$").unwrap()
});

pub fn matches_store_filter(launcher: &str, store: &str) -> bool {
    let l = launcher.to_ascii_lowercase();
    let s = store.to_ascii_lowercase();
    if s == "all" || s == "all games" || s.is_empty() {
        return true;
    }
    if s.contains("steam") {
        return l.contains("steam");
    }
    if s.contains("epic") {
        return l.contains("epic");
    }
    if s.contains("gog") {
        return l.contains("gog");
    }
    if s.contains("xbox") || s.contains("game pass") {
        return l.contains("xbox") || l.contains("game pass");
    }
    if s.contains("added by hand") || s.contains("manual") || s.contains("hand") || s.contains("custom") {
        return l.contains("added by hand") || l.contains("manual") || l.contains("hand") || l.contains("custom");
    }
    if s.contains("folder") || s.contains("my folders") {
        return l.contains("folder") || l.contains("custom") || (!l.contains("steam") && !l.contains("epic") && !l.contains("gog") && !l.contains("xbox"));
    }
    l == s || l.contains(&s) || s.contains(&l)
}

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
    if has("d3d12.dll") {
        return Some("DirectX 12".to_string());
    }

    // PE exports D3D12SDKVersion or D3D12SDKPath (Microsoft DirectX 12 Agility SDK)
    if let Some(pe) = inspect_pe(path) {
        if pe.exports.iter().any(|e| e == "D3D12SDKVersion" || e == "D3D12SDKPath") {
            return Some("DirectX 12".to_string());
        }
    }

    // D3D12 markers in binary (D3D12CreateDevice / D3D12SDKPath / D3D12SDKVersion)
    let d3d12_markers = &["D3D12CreateDevice", "D3D12SDKPath", "D3D12SDKVersion"];
    let found_d3d12 = find_markers(path, d3d12_markers);
    if !found_d3d12.is_empty() {
        return Some("DirectX 12".to_string());
    }

    // Directory contains DirectX 12 Agility SDK, DXC compiler, or Streamline / FSR Frame Generation DLLs
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

    // 3. DirectX 11 indicators:
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

pub fn is_generic_folder_name(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
        "bin" | "binaries" | "binary" | "x64" | "x86" | "win64" | "win32"
            | "release" | "shipping" | "game" | "games" | "app" | "build"
            | "retail" | "pc" | "system" | "engine"
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
            return exe_stem.replace('_', " ").replace('-', " ");
        }
    }

    if resolved_name.trim().is_empty() {
        "Unknown Game".to_string()
    } else {
        resolved_name
    }
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

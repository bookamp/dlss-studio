#![allow(dead_code)]

pub mod heuristics;
pub mod steam;
pub mod epic;
pub mod gog;
pub mod xbox;

use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use regex::Regex;

#[cfg(windows)]
use windows::Win32::System::Registry::{
    RegOpenKeyExW, RegQueryValueExW, RegEnumKeyExW, RegCloseKey,
    HKEY, REG_SAM_FLAGS, REG_VALUE_TYPE,
};
#[cfg(windows)]
use windows::core::{PCWSTR, PWSTR};

pub use heuristics::*;
pub use steam::discover_steam;
pub use epic::discover_epic;
pub use gog::discover_gog;
pub use xbox::discover_xbox;

use xbox::{declared_executables, extract_xbox_metadata};
use gog::extract_gog_metadata;
use crate::core::pe::inspect_pe;

#[cfg(windows)]
pub(crate) fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
pub(crate) fn win32_read_reg_string(root: HKEY, subkey: &str, value_name: &str, sam: REG_SAM_FLAGS) -> Option<String> {
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
pub(crate) fn win32_enum_subkeys(root: HKEY, subkey: &str, sam: REG_SAM_FLAGS) -> Vec<String> {
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
    /// Strictly detects nvngx_dlss.dll / _nvngx.dll files deployed by DLSS Studio mods (Feeder or OptiScaler).
    pub fn has_native_dlss(&self) -> bool {
        let has_sr_binary = self.dlss_version.is_some() || self.files.iter().any(|f| {
            let lower = f.rel.to_lowercase();
            lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll")
        });

        if !has_sr_binary {
            return false;
        }

        // Only if an active mod deployment is currently running on the game,
        // check if this active deployment newly added nvngx_dlss.dll to a non-DLSS game.
        if let Some(manifest) = crate::core::journal::read_manifest(&self.dir) {
            let was_added = manifest.added.iter().any(|a| {
                let lower = a.to_lowercase();
                lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll")
            });
            let was_replaced = manifest.replaced.iter().any(|r| {
                let lower = r.rel.to_lowercase();
                lower.ends_with("nvngx_dlss.dll") || lower.ends_with("_nvngx.dll")
            });
            if was_added && !was_replaced {
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
    if lower.contains("shipping.exe") || lower.ends_with("-shipping.exe") || lower.ends_with("_shipping.exe") || lower.contains("win64-shipping") || lower.contains("wingdk-shipping") {
        score += 15000;
    }
    if lower.contains("singleplayer") || lower.ends_with("sp.exe") || lower.contains("_sp") {
        score += 400;
    }
    if lower.contains("multiplayer") || lower.ends_with("mp.exe") || lower.contains("_mp") {
        score -= 400;
    }
    if lower.ends_with("game.exe") {
        score += 2000;
    }
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
    if c.depth <= 1 {
        score += 3000;
    }
    let api_lower = c.api.to_lowercase();
    if api_lower.contains("directx") || api_lower.contains("vulkan") || api_lower.contains("opengl") {
        score += 5000;
    }
    if c.size > 5_000_000 {
        score += 4000;
    }
    if c.size < 1_000_000 && !c.declared && !c.has_sibling_dlss {
        score -= 5000;
    }
    let norm_dir: String = dir_name.chars().filter(|ch| ch.is_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect();
    let norm_name: String = c.name.chars().filter(|ch| ch.is_alphanumeric()).flat_map(|ch| ch.to_lowercase()).collect();
    if !norm_dir.is_empty() && (norm_name.starts_with(&norm_dir) || norm_dir.starts_with(&norm_name)) {
        score += 8000;
    }
    score += playable_role_score(&c.name, &c.rel);
    score
}

pub fn scan_game_directory<P: AsRef<Path>>(dir: P) -> Option<GameEntry> {
    let clean_dir = crate::core::state::clean_path_separators(dir.as_ref());
    let dir = clean_dir.as_path();
    if !dir.exists() || !dir.is_dir() {
        return None;
    }

    let mut dlss_files: Vec<(PathBuf, Option<String>)> = Vec::new();
    let mut fg_files: Vec<(PathBuf, Option<String>)> = Vec::new();
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
            } else if (file_name == "nvngx_dlssg.dll" || file_name == "sl.dlss_g.dll" || file_name.contains("framegeneration_dx12")) && !is_inside_mod_dir {
                has_fg = true;
                let pe_opt = inspect_pe(path);
                let ver = pe_opt.and_then(|p| p.version);
                fg_files.push((path.to_path_buf(), ver.clone()));
                dlss_files.push((path.to_path_buf(), ver));
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

    let mut nr_style: usize = 0;
    for s_dir in search_dirs.into_iter().flatten() {
        let reshade_ini = s_dir.join("ReShade.ini");
        if reshade_ini.is_file() {
            if let Ok(text) = fs::read_to_string(&reshade_ini) {
                if let Some(val) = crate::core::ini::get_ini(&text, "RenoDX.DLSS5", "NRStyle") {
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
                if let Some(val) = crate::core::ini::get_ini(&text, "RenoDX.DLSS5", "NRStyle") {
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
                if let Some(val) = crate::core::ini::get_ini(&text, "DlssNr", "Style") {
                    if let Ok(parsed) = val.trim().parse::<usize>() {
                        nr_style = parsed;
                        break;
                    }
                }
            }
        }
    }

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
        heuristics::infer_game_name(dir, &chosen.path, xbox_name)
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

    let is_mod_added_dlss = if let Some((sr_p, _)) = &sr_file {
        let rel = sr_p.strip_prefix(dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| sr_p.file_name().unwrap_or_default().to_string_lossy().to_string());

        let in_active_added = crate::core::journal::read_manifest(dir)
            .map(|m| m.added.iter().any(|a| a.eq_ignore_ascii_case(&rel)))
            .unwrap_or(false);

        let was_replaced = crate::core::journal::read_manifest(dir)
            .map(|m| m.replaced.iter().any(|r| r.rel.eq_ignore_ascii_case(&rel)))
            .unwrap_or(false);

        in_active_added && !was_replaced
    } else {
        false
    };
    let has_native_dlss = sr_file.is_some() && !is_mod_added_dlss;

    let fg_file = fg_files.iter().find(|(p, _)| {
        let n = p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        n == "nvngx_dlssg.dll" || n == "sl.dlss_g.dll" || n.contains("framegeneration_dx12")
    });

    let is_mod_added_fg = if let Some((fg_p, _)) = &fg_file {
        let rel = fg_p.strip_prefix(dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| fg_p.file_name().unwrap_or_default().to_string_lossy().to_string());

        let in_active_added = crate::core::journal::read_manifest(dir)
            .map(|m| m.added.iter().any(|a| a.eq_ignore_ascii_case(&rel)))
            .unwrap_or(false);

        let was_replaced = crate::core::journal::read_manifest(dir)
            .map(|m| m.replaced.iter().any(|r| r.rel.eq_ignore_ascii_case(&rel)))
            .unwrap_or(false);

        in_active_added && !was_replaced
    } else {
        false
    };

    let api_lower = chosen.api.to_lowercase();
    let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");
    let is_vulkan = api_lower.contains("vulkan");
    let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !is_dx12;

    let has_native_fg = fg_file.is_some() && !is_mod_added_fg && !is_vulkan && !is_dx11;
    let can_inject_fg = chosen.bitness == 64 && has_native_dlss && !has_native_fg && (is_dx12 || is_dx11);
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
        has_frame_generation: has_native_fg,
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

        for i in 2..26 {
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

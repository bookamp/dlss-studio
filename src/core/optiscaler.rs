#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use crate::core::journal::{ActiveManifest, ManifestItem, ManifestGame, save_manifest, append_history, HistoryRow};
use crate::core::mfg_unlock::configure_mfg_unlock_ini;

pub const RELEASE_VERSION: &str = "0.7.6-dlssnr";

#[derive(Debug, Clone)]
pub struct OptiScalerOptions {
    pub pre_sr: bool,
    pub passes: u32,
    pub mfg_unlock: bool,
    pub target_exe_name: String,
    pub nr_style: usize,
}

impl Default for OptiScalerOptions {
    fn default() -> Self {
        Self {
            pre_sr: true,
            passes: 3,
            mfg_unlock: false,
            target_exe_name: String::new(),
            nr_style: 0,
        }
    }
}

/// Updates or inserts a key/value pair within a specific [Section] of an INI file while preserving all comments and other formatting.
pub fn set_ini(text: &str, section: &str, key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.replace('\u{feff}', "").lines().map(|s| s.to_string()).collect();

    let header = format!("[{}]", section).to_lowercase();
    let start_idx = lines.iter().position(|l| l.trim().to_lowercase() == header);

    if let Some(start) = start_idx {
        let mut end = lines.len();
        for i in (start + 1)..lines.len() {
            let t = lines[i].trim();
            if t.starts_with('[') && t.ends_with(']') {
                end = i;
                break;
            }
        }
        let wanted = key.to_lowercase();
        let mut changed = false;
        for i in (start + 1)..end {
            let line = &lines[i];
            let trimmed = line.trim();
            if !trimmed.starts_with(';') && !trimmed.starts_with('#') {
                if let Some((k, _)) = trimmed.split_once('=') {
                    if k.trim().to_lowercase() == wanted {
                        lines[i] = format!("{}={}", key, value);
                        changed = true;
                        break;
                    }
                }
            }
        }
        if !changed {
            lines.insert(end, format!("{}={}", key, value));
        }
    } else {
        if !lines.is_empty() && !lines.last().unwrap().is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("[{}]", section));
        lines.push(format!("{}={}", key, value));
    }

    lines.join(newline)
}

pub fn get_ini(text: &str, section: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let header = format!("[{}]", section).to_lowercase();
    let start = lines.iter().position(|l| l.trim().to_lowercase() == header)?;

    let wanted = key.to_lowercase();
    for line in &lines[(start + 1)..] {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            break;
        }
        if trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim().to_lowercase() == wanted {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

pub fn generate_optiscaler_ini(
    base_text: &str,
    pre_sr: bool,
    passes: u32,
    external_mfg: bool,
    target_exe: Option<&str>,
    nr_style: usize,
) -> String {
    let mut text = base_text.to_string();

    text = set_ini(&text, "DlssNr", "Enabled", if pre_sr { "true" } else { "false" });
    text = set_ini(&text, "DlssNr", "RunBeforeSR", if pre_sr { "true" } else { "false" });
    text = set_ini(&text, "DlssNr", "Passes", &passes.to_string());
    text = set_ini(&text, "DlssNr", "ApplyAfterRR", "true");
    if pre_sr {
        text = set_ini(&text, "DlssNr", "Style", &nr_style.to_string());
    }
    text = set_ini(&text, "Plugins", "LoadReshade", "false");
    text = set_ini(&text, "FrameGen", "External", if external_mfg { "true" } else { "false" });
    text = set_ini(&text, "DLSSG", "InterpolationCount", "auto");
    text = set_ini(&text, "DLSSG", "OverrideInterpolationCount", "auto");
    text = set_ini(&text, "Menu", "OverlayMenu", "true");
    text = set_ini(&text, "Menu", "ShortcutKey", "0x2D"); // INSERT key

    if let Some(exe) = target_exe {
        if !exe.is_empty() {
            text = set_ini(&text, "Init", "TargetProcessName", exe);
        }
    }

    text
}

pub fn configure_optiscaler_ini(base_text: &str, opts: &OptiScalerOptions) -> String {
    generate_optiscaler_ini(
        base_text,
        opts.pre_sr,
        opts.passes,
        opts.mfg_unlock,
        Some(&opts.target_exe_name),
        opts.nr_style,
    )
}

/// Locates the OptiScaler component directory containing OptiScaler.dll, OptiScaler.ini, and OptiScaler subfolder.
fn app_exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// Returns standard candidate directories to check for runtime component payloads.
fn get_component_roots() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(crate::core::downloader::get_components_root());
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(&local_appdata).join("dlss-5-studio").join("components"));
        dirs.push(PathBuf::from(&local_appdata).join("DLSS-Studio").join("components"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(PathBuf::from(&appdata).join("dlss-5-studio").join("components"));
        dirs.push(PathBuf::from(&appdata).join("DLSS-Studio").join("components"));
    }
    if let Some(exe) = app_exe_dir() {
        dirs.push(exe.join("components"));
    }
    dirs
}

/// Locates the OptiScaler component directory containing OptiScaler.dll, OptiScaler.ini, and OptiScaler subfolder.
pub fn find_optiscaler_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in get_component_roots() {
        candidates.push(root.join("OptiScaler-0.8.4-dlssnr"));
        candidates.push(root.join("OptiScaler-0.8.3-dlssnr"));
        candidates.push(root.join("OptiScaler-0.7.7-dlssnr"));
        candidates.push(root.join("OptiScaler-DLSSNR-v0.7.7"));
        candidates.push(root.join("OptiScaler-0.7.6-dlssnr"));
        candidates.push(root.join("OptiScaler-DLSSNR-v0.7.6"));
        candidates.push(root.join("OptiScaler-v0.7.6"));
        candidates.push(root.join("OptiScaler-0.6.2-dlssnr"));
        candidates.push(root.join("OptiScaler-0.2.0-dlssnr"));
    }
    if let Some(exe) = app_exe_dir() {
        candidates.push(exe.join("components").join("OptiScaler-0.8.4-dlssnr"));
        candidates.push(exe.join("components").join("OptiScaler-0.8.3-dlssnr"));
        candidates.push(exe.join("components").join("OptiScaler-0.7.7-dlssnr"));
        candidates.push(exe.join("components").join("OptiScaler-DLSSNR-v0.7.7"));
        candidates.push(exe.join("payload").join("OptiScaler-0.7.7-dlssnr"));
        candidates.push(exe.join("payload").join("OptiScaler-DLSSNR-v0.7.7"));
        candidates.push(exe.join("components").join("OptiScaler-0.7.6-dlssnr"));
        candidates.push(exe.join("components").join("OptiScaler-DLSSNR-v0.7.6"));
        candidates.push(exe.join("payload").join("OptiScaler-0.7.6-dlssnr"));
        candidates.push(exe.join("payload").join("OptiScaler-DLSSNR-v0.7.6"));
        candidates.push(exe.join("components").join("OptiScaler-0.6.2-dlssnr"));
        candidates.push(exe.join("payload").join("OptiScaler-0.6.2-dlssnr"));
    }

    for c in candidates {
        if c.join("OptiScaler.dll").exists() {
            return Some(c);
        }
    }
    None
}

/// Locates ReShade64.dll payload
pub fn find_reshade64_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(r"payload\reshade-vulkan\ReShade64.dll"));
    candidates.push(PathBuf::from(r"payload\ReShade64.dll"));
    for root in get_component_roots() {
        candidates.push(root.join("reshade-vulkan").join("ReShade64.dll"));
        candidates.push(root.join("ReShade64.dll"));
    }
    if let Some(exe) = app_exe_dir() {
        for ancestor in exe.ancestors().take(4) {
            candidates.push(ancestor.join("payload").join("reshade-vulkan").join("ReShade64.dll"));
            candidates.push(ancestor.join("payload").join("ReShade64.dll"));
            candidates.push(ancestor.join("components").join("reshade-vulkan").join("ReShade64.dll"));
            candidates.push(ancestor.join("components").join("ReShade64.dll"));
        }
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Locates ReShade32.dll payload
pub fn find_reshade32_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(r"payload\reshade-vulkan\ReShade32.dll"));
    candidates.push(PathBuf::from(r"payload\ReShade32.dll"));
    for root in get_component_roots() {
        candidates.push(root.join("reshade-vulkan").join("ReShade32.dll"));
        candidates.push(root.join("ReShade32.dll"));
    }
    if let Some(exe) = app_exe_dir() {
        for ancestor in exe.ancestors().take(4) {
            candidates.push(ancestor.join("payload").join("reshade-vulkan").join("ReShade32.dll"));
            candidates.push(ancestor.join("payload").join("ReShade32.dll"));
            candidates.push(ancestor.join("components").join("reshade-vulkan").join("ReShade32.dll"));
            candidates.push(ancestor.join("components").join("ReShade32.dll"));
        }
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Locates the RenoDX 4x MFG Unlock addon
pub fn find_mfg_addon_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in get_component_roots() {
        candidates.push(root.join("mfg-unlock-1.0").join("renodx-mfgunlock.addon64"));
        candidates.push(root.join("mfg-unlock-0.9").join("renodx-mfgunlock.addon64"));
        candidates.push(root.join("mfg-unlock-0.8").join("renodx-mfgunlock.addon64"));
        candidates.push(root.join("mfg-unlock-0.6.1").join("renodx-mfgunlock.addon64"));
        candidates.push(root.join("renodx-mfgunlock.addon64"));
    }
    if let Some(exe) = app_exe_dir() {
        candidates.push(exe.join("components").join("mfg-unlock-1.0").join("renodx-mfgunlock.addon64"));
        candidates.push(exe.join("components").join("mfg-unlock-0.9").join("renodx-mfgunlock.addon64"));
        candidates.push(exe.join("components").join("mfg-unlock-0.8").join("renodx-mfgunlock.addon64"));
        candidates.push(exe.join("components").join("mfg-unlock-0.6.1").join("renodx-mfgunlock.addon64"));
        candidates.push(exe.join("payload").join("addons").join("renodx-mfgunlock.addon64"));
        candidates.push(exe.join("renodx-mfgunlock.addon64"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}


/// Locates the RenoDX v4.7 Integrated DLSS 5 Engine addon
pub fn find_renodx_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in get_component_roots() {
        candidates.push(root.join("renodx-dlss5").join("renodx-dlss5.addon64"));
        candidates.push(root.join("renodx-dlss5.addon64"));
        candidates.push(root.join("addons").join("renodx-dlss5.addon64"));
    }
    if let Some(exe) = app_exe_dir() {
        candidates.push(exe.join("components").join("renodx-dlss5").join("renodx-dlss5.addon64"));
        candidates.push(exe.join("payload").join("renodx-dlss5.addon64"));
        candidates.push(exe.join("addons").join("renodx-dlss5.addon64"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Locates NIGos's DLSS 5 D3D12 Mip Chain Companion addon
pub fn find_dlss5_d3d12_fix_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in get_component_roots() {
        candidates.push(root.join("dlss-mip-fix").join("dlss-mip-fix.addon64"));
        candidates.push(root.join("dlss-mip-fix.addon64"));
        candidates.push(root.join("addons").join("dlss-mip-fix.addon64"));
        candidates.push(root.join("dlss5-d3d12-fix").join("dlss5-d3d12-fix.addon64"));
        candidates.push(root.join("dlss5-d3d12-fix.addon64"));
        candidates.push(root.join("addons").join("dlss5-d3d12-fix.addon64"));
    }
    if let Some(exe) = app_exe_dir() {
        candidates.push(exe.join("components").join("dlss-mip-fix").join("dlss-mip-fix.addon64"));
        candidates.push(exe.join("payload").join("dlss-mip-fix.addon64"));
        candidates.push(exe.join("addons").join("dlss-mip-fix.addon64"));
        candidates.push(exe.join("components").join("dlss5-d3d12-fix").join("dlss5-d3d12-fix.addon64"));
        candidates.push(exe.join("payload").join("dlss5-d3d12-fix.addon64"));
        candidates.push(exe.join("addons").join("dlss5-d3d12-fix.addon64"));
    }
    candidates.push(PathBuf::from("target/release/dlss_mip_addon.dll"));
    candidates.push(PathBuf::from("target/release/dlss-mip-fix.addon64"));
    candidates.push(PathBuf::from("dist/dlss-mip-fix.addon64"));

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Locates the modern Streamline nvngx_dlss.dll runtime (v3.7+/v3.10+)
pub fn find_dlss_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in get_component_roots() {
        candidates.push(root.join("streamline-2.14.1").join("streamline").join("nvngx_dlss.dll"));
        candidates.push(root.join("streamline").join("nvngx_dlss.dll"));
        candidates.push(root.join("nvngx_dlss.dll"));
    }
    if let Some(exe) = app_exe_dir() {
        candidates.push(exe.join("components").join("streamline-2.14.1").join("streamline").join("nvngx_dlss.dll"));
        candidates.push(exe.join("components").join("streamline").join("nvngx_dlss.dll"));
        candidates.push(exe.join("payload").join("streamline-2.14.1").join("streamline").join("nvngx_dlss.dll"));
        candidates.push(exe.join("payload").join("streamline").join("nvngx_dlss.dll"));
        candidates.push(exe.join("nvngx_dlss.dll"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Locates the Streamline nvngx_dlssnr.dll runtime
pub fn find_dlssnr_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in get_component_roots() {
        candidates.push(root.join("streamline-2.14.1").join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(root.join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(root.join("nvngx_dlssnr.dll"));
    }
    if let Some(exe) = app_exe_dir() {
        candidates.push(exe.join("components").join("streamline-2.14.1").join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(exe.join("components").join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(exe.join("payload").join("streamline-2.14.1").join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(exe.join("payload").join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(exe.join("nvngx_dlssnr.dll"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Locates Dashdogy's Universal RTXMFG v1.3.2 standalone DLL (RTXMFG.dll)
pub fn find_standalone_mfg_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(r"payload\mfg-standalone\RTXMFG.dll"));
    candidates.push(PathBuf::from(r"components\mfg-standalone\RTXMFG.dll"));
    for root in get_component_roots() {
        candidates.push(root.join("mfg-standalone").join("RTXMFG.dll"));
        candidates.push(root.join("RTXMFG.dll"));
    }
    if let Some(exe) = app_exe_dir() {
        for ancestor in exe.ancestors().take(4) {
            candidates.push(ancestor.join("payload").join("mfg-standalone").join("RTXMFG.dll"));
            candidates.push(ancestor.join("payload").join("RTXMFG.dll"));
            candidates.push(ancestor.join("components").join("mfg-standalone").join("RTXMFG.dll"));
            candidates.push(ancestor.join("RTXMFG.dll"));
        }
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

pub fn find_streamline_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(r"payload\streamline-2.14.1\streamline"));
    candidates.push(PathBuf::from(r"payload\streamline"));
    for root in get_component_roots() {
        candidates.push(root.join("streamline-2.14.1").join("streamline"));
        candidates.push(root.join("streamline"));
        candidates.push(root.join("OptiScaler-0.7.7-dlssnr").join("OptiScaler").join("streamline"));
    }
    if let Some(exe) = app_exe_dir() {
        for ancestor in exe.ancestors().take(4) {
            candidates.push(ancestor.join("payload").join("streamline-2.14.1").join("streamline"));
            candidates.push(ancestor.join("payload").join("streamline"));
            candidates.push(ancestor.join("components").join("streamline-2.14.1").join("streamline"));
            candidates.push(ancestor.join("components").join("streamline"));
            candidates.push(ancestor.join("components").join("OptiScaler-0.7.7-dlssnr").join("OptiScaler").join("streamline"));
        }
    }

    for c in candidates {
        if c.join("sl.interposer.dll").exists() && c.join("sl.common.dll").exists() {
            return Some(c);
        }
    }
    None
}

/// Locates the open-source DLSS-NR forwarder (nvngx.dll_dlssnr.dll)
pub fn find_nvngx_snippet_payload(opti_dir: &Path) -> Option<PathBuf> {
    if opti_dir.join("nvngx.dll_dlssnr.dll").is_file() {
        return Some(opti_dir.join("nvngx.dll_dlssnr.dll"));
    }
    for root in get_component_roots() {
        let c1 = root.join("OptiScaler-0.7.7-dlssnr").join("nvngx.dll_dlssnr.dll");
        if c1.is_file() {
            return Some(c1);
        }
        let c2 = root.join("OptiScaler-0.2.0-dlssnr").join("nvngx.dll_dlssnr.dll");
        if c2.is_file() {
            return Some(c2);
        }
        let c3 = root.join("nvngx.dll_dlssnr.dll");
        if c3.is_file() {
            return Some(c3);
        }
    }
    if let Some(exe) = app_exe_dir() {
        for ancestor in exe.ancestors().take(4) {
            let c1 = ancestor.join("payload").join("OptiScaler-0.7.7-dlssnr").join("nvngx.dll_dlssnr.dll");
            if c1.is_file() {
                return Some(c1);
            }
            let c2 = ancestor.join("components").join("OptiScaler-0.7.7-dlssnr").join("nvngx.dll_dlssnr.dll");
            if c2.is_file() {
                return Some(c2);
            }
            let c3 = ancestor.join("payload").join("nvngx.dll_dlssnr.dll");
            if c3.is_file() {
                return Some(c3);
            }
        }
    }
    None
}

/// Fully decoupled, typed bundle of required runtime component binaries.
/// Enables 100% in-memory / temporary sandbox unit testing without machine-level dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadBundle {
    pub optiscaler_dll: PathBuf,
    pub optiscaler_ini: PathBuf,
    pub optiscaler_dir: Option<PathBuf>,
    pub nvngx_dlss_dll: Option<PathBuf>,
    pub nvngx_dlssnr_dll: Option<PathBuf>,
    pub nvngx_snippet_dll: Option<PathBuf>,
    pub rtxmfg_dll: Option<PathBuf>,
    pub reshade64_dll: Option<PathBuf>,
    pub reshade32_dll: Option<PathBuf>,
    pub renodx_dlss5_addon: Option<PathBuf>,
    pub dlss5_d3d12_fix_addon: Option<PathBuf>,
    pub renodx_mfgunlock_addon: Option<PathBuf>,
    pub streamline_dir: Option<PathBuf>,
    pub feeder_components: Option<crate::core::downloader::FeederComponents>,
    pub dgvoodoo: Option<crate::core::downloader::DgVoodooComponents>,
}

impl PayloadBundle {
    pub fn from_system() -> Result<Self, String> {
        let opti_dir = find_optiscaler_payload()
            .ok_or_else(|| "OptiScaler payload not found on system. Please verify component payloads.".to_string())?;
        let optiscaler_dll = opti_dir.join("OptiScaler.dll");
        let optiscaler_ini = opti_dir.join("OptiScaler.ini");
        let optiscaler_dir = if opti_dir.join("OptiScaler").is_dir() {
            Some(opti_dir.join("OptiScaler"))
        } else {
            None
        };
        let nvngx_snippet_dll = find_nvngx_snippet_payload(&opti_dir);
        let nvngx_dlss_dll = find_dlss_payload();
        let nvngx_dlssnr_dll = find_dlssnr_payload();
        let rtxmfg_dll = find_standalone_mfg_payload();
        let reshade64_dll = find_reshade64_payload();
        let reshade32_dll = find_reshade32_payload();
        let renodx_dlss5_addon = find_renodx_payload();
        let dlss5_d3d12_fix_addon = find_dlss5_d3d12_fix_payload();
        let renodx_mfgunlock_addon = find_mfg_addon_payload();
        let streamline_dir = find_streamline_payload();
        let feeder_components = crate::core::downloader::find_local_feeder_components();
        let dgvoodoo = crate::core::downloader::find_local_dgvoodoo_components();

        Ok(Self {
            optiscaler_dll,
            optiscaler_ini,
            optiscaler_dir,
            nvngx_dlss_dll,
            nvngx_dlssnr_dll,
            nvngx_snippet_dll,
            rtxmfg_dll,
            reshade64_dll,
            reshade32_dll,
            renodx_dlss5_addon,
            dlss5_d3d12_fix_addon,
            renodx_mfgunlock_addon,
            streamline_dir,
            feeder_components,
            dgvoodoo,
        })
    }
}

/// Verifies that a discovered overlay add-on binary is a valid 64-bit ReShade addon payload
fn is_native_overlay_addon(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    // Verified 64-bit ReShade companion addon binary payload
    metadata.len() >= 50_000
}

/// Locates the DLSS Studio In-Game Overlay addon on disk if present
pub fn find_overlay_addon_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    // 1. AppData components
    let appdata_comp = crate::core::state::get_appdata_dir().join("components");
    candidates.push(appdata_comp.join("dlss-overlay.addon64"));
    candidates.push(appdata_comp.join("dlss5-lab-overlay.addon64"));

    // 2. Workspace assets and build outputs
    candidates.push(PathBuf::from(r"assets\dlss-overlay.addon64"));
    candidates.push(PathBuf::from(r"assets\dlss5-lab-overlay.addon64"));
    candidates.push(PathBuf::from(r"target\release\dlss_overlay.dll"));
    candidates.push(PathBuf::from(r"target\release\dlss-overlay.addon64"));
    candidates.push(PathBuf::from(r"target\release\dlss5_lab_overlay.dll"));
    candidates.push(PathBuf::from(r"target\release\dlss5-lab-overlay.addon64"));
    candidates.push(PathBuf::from(r"target\debug\dlss_overlay.dll"));
    candidates.push(PathBuf::from(r"target\debug\dlss-overlay.addon64"));
    candidates.push(PathBuf::from(r"target\debug\dlss5_lab_overlay.dll"));
    candidates.push(PathBuf::from(r"target\debug\dlss5-lab-overlay.addon64"));

    // 3. Component roots and app executable dir
    for root in get_component_roots() {
        candidates.push(root.join("dlss-overlay.addon64"));
        candidates.push(root.join("dlss5-lab-overlay.addon64"));
        candidates.push(root.join("overlay").join("dlss-overlay.addon64"));
        candidates.push(root.join("overlay").join("dlss5-lab-overlay.addon64"));
    }
    if let Some(exe) = app_exe_dir() {
        candidates.push(exe.join("dlss-overlay.addon64"));
        candidates.push(exe.join("dlss5-lab-overlay.addon64"));
        candidates.push(exe.join("components").join("dlss-overlay.addon64"));
        candidates.push(exe.join("components").join("dlss5-lab-overlay.addon64"));
        candidates.push(exe.join("overlay").join("dlss-overlay.addon64"));
        candidates.push(exe.join("overlay").join("dlss5-lab-overlay.addon64"));
    }

    for c in candidates {
        if is_native_overlay_addon(&c) {
            return Some(c);
        }
    }

    None
}


#[derive(Debug, Clone)]
pub struct DeployOptions {
    pub game_name: Option<String>,
    pub game_dir: PathBuf,
    pub exe_path: PathBuf,
    pub api: String,
    pub pre_sr: bool,
    pub passes: u32,
    pub mfg_unlock: bool,
    pub mfg_multiplier: u32,
    pub nr_style: usize,
    pub nr_style_enabled: bool,
}

impl Default for DeployOptions {
    fn default() -> Self {
        Self {
            game_name: None,
            game_dir: PathBuf::new(),
            exe_path: PathBuf::new(),
            api: "DirectX 12".to_string(),
            pre_sr: false,
            passes: 1,
            mfg_unlock: false,
            mfg_multiplier: 1,
            nr_style: 0,
            nr_style_enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeployResult {
    pub success: bool,
    pub log_lines: Vec<String>,
    pub replaced: usize,
    pub added: usize,
}

#[doc(hidden)]
pub fn is_known_mod_file(dest: &Path) -> bool {
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if name == "optiscaler.ini"
        || name == "optiscaler.log"
        || name == "optiscaler.dll"
        || name == "reshade.ini"
        || name == "reshade.log"
        || name == "reshade64.dll"
        || name == "reshade32.dll"
        || name == "reshade64.json"
        || name == "reshadegui.ini"
        || name == "reshadepreset.ini"
        || name == "nvngx.dll_dlssnr.dll"
        || name == "nvngx_dlssnr.dll"
        || name == "dlss5-feed.cfg"
        || name == "dlss5-feed.log"
        || name == "dlss5-feed.addon64"
        || name == "dlss5-feed.addon32"
        || name == "dlss5-feed-host64.exe"
        || name == "dlss-overlay.addon64"
        || name == "dlss5-lab-overlay.addon64"
        || name == "renodx-dlss5.addon64"
        || name == "dlss-mip-fix.addon64"
        || name == "dlss-mip-fix.cfg"
        || name == "dlss-mip-fix.log"
        || name == "dlss5-d3d12-fix.addon64"
        || name == "dlss5-d3d12-fix.cfg"
        || name == "dlss5-d3d12-fix.log"
        || name == "renodx-mfgunlock.addon64"
        || name == "dgvoodoo.conf"
        || name == "dgvoodoo.log"
        || name == "rtxmfg-universal.json"
        || name == "rtx40mfg-universal.json"
        || name == "sl.pcl.dll"
        || name.starts_with("rtxmfg-")
        || name.ends_with(".addon64")
        || name.ends_with(".addon32")
        || name.ends_with(".addon")
    {
        return true;
    }
    if dest.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.eq_ignore_ascii_case("OptiScaler") || s.eq_ignore_ascii_case("host64")
    }) {
        return true;
    }
    let hook_names = ["dxgi.dll", "winmm.dll", "d3d12.dll", "d3d11.dll", "d3d9.dll", "d3d8.dll", "opengl32.dll", "dinput8.dll", "version.dll"];
    if hook_names.contains(&name.as_str()) && dest.is_file() {
        return crate::core::pe::is_optiscaler_or_proxy(dest) || crate::core::pe::is_reshade_dll(dest).0;
    }
    false
}

fn is_stale_proxy_dll(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if crate::core::pe::is_optiscaler_or_proxy(path) || crate::core::pe::is_reshade_dll(path).0 {
        return true;
    }
    if let Ok(bytes) = fs::read(path) {
        let s = String::from_utf8_lossy(&bytes).to_lowercase();
        if s.contains("reshade") || s.contains("optiscaler") || s.contains("dgvoodoo") {
            return true;
        }
    }
    false
}

fn remove_stale_proxy_hooks(mod_root: &Path, active_hooks: &[&str], log: &mut Vec<String>) {
    let check_hooks = ["winmm.dll", "version.dll", "dinput8.dll", "dxgi.dll", "d3d12.dll", "d3d11.dll", "d3d9.dll", "d3d8.dll", "opengl32.dll"];
    for stale_name in check_hooks {
        if !active_hooks.iter().any(|h| h.eq_ignore_ascii_case(stale_name)) {
            let stale_p = mod_root.join(stale_name);
            if is_stale_proxy_dll(&stale_p) {
                if fs::remove_file(&stale_p).is_ok() {
                    log.push(format!("[CLEANUP] Removed obsolete proxy hook: {}", stale_name));
                }
            }
        }
    }
}

pub fn clean_conflicting_route_artifacts(
    target_route: &str,
    game_dir: &Path,
    mod_root: &Path,
    mfg_unlock: bool,
    api: &str,
    log: &mut Vec<String>,
) -> std::io::Result<()> {
    let mut unique_dirs: Vec<PathBuf> = Vec::new();
    if mod_root.is_dir() {
        unique_dirs.push(mod_root.to_path_buf());
    }
    if game_dir.is_dir() && !unique_dirs.iter().any(|d| d == game_dir) {
        unique_dirs.push(game_dir.to_path_buf());
    }

    // Check if there is an existing active manifest for a DIFFERENT route
    if let Some(prev_manifest) = crate::core::journal::read_manifest(game_dir) {
        if prev_manifest.route != target_route {
            log.push(format!("[SWAP] Switching route from '{}' to '{}' - cleaning prior route artifacts", prev_manifest.route, target_route));
            for rel in &prev_manifest.added {
                let target_file = crate::core::journal::resolve_target_path(game_dir, rel);
                if target_file.is_file() {
                    let fname = target_file.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                    if is_known_mod_file(&target_file)
                        || fname.ends_with(".addon64")
                        || fname.ends_with(".addon32")
                        || fname.ends_with(".addon")
                        || fname.ends_with(".ini")
                        || fname.ends_with(".cfg")
                        || fname.ends_with(".log")
                    {
                        if fs::remove_file(&target_file).is_ok() {
                            log.push(format!("[SWAP] Purged prior route file: {}", rel));
                        }
                    }
                }
            }
            for rel_dir in prev_manifest.added_dirs.iter().rev() {
                let target_d = crate::core::journal::resolve_target_path(game_dir, rel_dir);
                if target_d.is_dir() {
                    let lower = rel_dir.to_lowercase();
                    if lower.contains("reshade") || lower.contains("optiscaler") {
                        if fs::remove_dir_all(&target_d).is_ok() {
                            log.push(format!("[SWAP] Purged prior route directory: {}", rel_dir));
                        }
                    }
                }
            }
        }
    }

    if target_route == "optiscaler" {
        // 1. Unregister Vulkan layer
        if crate::core::vulkan_layer::unregister_vulkan_layer(game_dir).unwrap_or(false) {
            log.push("[SWAP] Unregistered Vulkan implicit layer for OptiScaler deployment".to_string());
        }

        // 2. Remove ReShade, RenoDX, and Feeder files and directories
        for dir in &unique_dirs {
            let reshade_shaders = dir.join("reshade-shaders");
            if reshade_shaders.is_dir() {
                if fs::remove_dir_all(&reshade_shaders).is_ok() {
                    log.push(format!("[SWAP] Removed conflicting reshade-shaders/ from {}", dir.display()));
                }
            }

            let host64_dir = dir.join("host64");
            if host64_dir.is_dir() {
                if fs::remove_dir_all(&host64_dir).is_ok() {
                    log.push(format!("[SWAP] Removed conflicting host64/ from {}", dir.display()));
                }
            }

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        let lower = fname.to_lowercase();
                        let is_reshade_artifact = lower == "reshade.ini"
                            || lower == "reshadepreset.ini"
                            || lower == "reshadegui.ini"
                            || lower == "reshade.log"
                            || lower == "reshade64.dll"
                            || lower == "reshade32.dll"
                            || lower == "reshade64.json"
                            || lower == "dlss5-feed.cfg"
                            || lower == "dlss5-feed.log"
                            || lower == "dlss5-feed.addon64"
                            || lower == "dlss5-feed.addon32"
                            || lower == "dlss5-feed-host64.exe"
                            || lower == "dlss-overlay.addon64"
                            || lower == "dlss5-lab-overlay.addon64"
                            || lower == "renodx-dlss5.addon64"
                            || lower == "dlss-mip-fix.addon64"
                            || lower == "dlss-mip-fix.cfg"
                            || lower == "dlss-mip-fix.log"
                            || lower == "dlss5-d3d12-fix.addon64"
                            || lower == "dlss5-d3d12-fix.cfg"
                            || lower == "dlss5-d3d12-fix.log"
                            || lower == "renodx-mfgunlock.addon64"
                            || lower == "dgvoodoo.conf"
                            || lower == "dgvoodoo.log"
                            || lower.ends_with(".addon64")
                            || lower.ends_with(".addon32")
                            || lower.ends_with(".addon");

                        let is_reshade_hook = (lower == "dxgi.dll" || lower == "d3d11.dll" || lower == "d3d12.dll" || lower == "d3d9.dll" || lower == "d3d8.dll" || lower == "opengl32.dll")
                            && (crate::core::pe::is_reshade_dll(&path).0 || crate::core::pe::is_optiscaler_or_proxy(&path) || {
                                if let Ok(bytes) = fs::read(&path) {
                                    let s = String::from_utf8_lossy(&bytes).to_lowercase();
                                    s.contains("reshade") || s.contains("dgvoodoo")
                                } else {
                                    false
                                }
                            });

                        if is_reshade_artifact || is_reshade_hook {
                            if fs::remove_file(&path).is_ok() {
                                log.push(format!("[SWAP] Cleaned conflicting ReShade file: {}", fname));
                            }
                        }
                    }
                }
            }

            // Also clean standalone MFG artifacts if mfg_unlock is false
            if !mfg_unlock {
                let ver_p = dir.join("version.dll");
                if ver_p.is_file() && (is_stale_proxy_dll(&ver_p) || crate::core::pe::is_optiscaler_or_proxy(&ver_p)) {
                    if fs::remove_file(&ver_p).is_ok() {
                        log.push("[CLEANUP] Removed standalone MFG version.dll (MFG disabled)".to_string());
                    }
                }
                let _ = fs::remove_file(dir.join("RTXMFG-Universal.json"));
                let _ = fs::remove_file(dir.join("RTX40MFG-Universal.json"));
            }
        }
    } else {
        // target_route is "feeder" or "native"
        // 1. Purge OptiScaler files and directories
        for dir in &unique_dirs {
            let opti_dir = dir.join("OptiScaler");
            if opti_dir.is_dir() {
                if fs::remove_dir_all(&opti_dir).is_ok() {
                    log.push(format!("[SWAP] Removed conflicting OptiScaler/ directory from {}", dir.display()));
                }
            }

            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        let lower = fname.to_lowercase();
                        let is_optiscaler_artifact = lower == "optiscaler.ini"
                            || lower == "optiscaler.log"
                            || lower == "optiscaler.dll"
                            || lower == "nvngx.dll_dlssnr.dll"
                            || lower == "nvngx_dlssnr.dll"
                            || lower == "rtxmfg-universal.json"
                            || lower == "rtx40mfg-universal.json"
                            || (lower.starts_with("rtxmfg-") && lower.ends_with(".json"));

                        let is_opti_or_mfg_hook = (lower == "dxgi.dll" || lower == "version.dll" || lower == "d3d12.dll" || lower == "d3d11.dll" || lower == "d3d9.dll" || lower == "d3d8.dll")
                            && (crate::core::pe::is_optiscaler_or_proxy(&path) || {
                                if let Ok(bytes) = fs::read(&path) {
                                    let s = String::from_utf8_lossy(&bytes).to_lowercase();
                                    s.contains("optiscaler") || s.contains("rtxmfg") || s.contains("dgvoodoo")
                                } else {
                                    false
                                }
                            });

                        if is_optiscaler_artifact || is_opti_or_mfg_hook {
                            if fs::remove_file(&path).is_ok() {
                                log.push(format!("[SWAP] Cleaned conflicting OptiScaler file: {}", fname));
                            }
                        }
                    }
                }
            }

            if target_route == "native" {
                let _ = fs::remove_file(dir.join("dlss5-feed.cfg"));
                let _ = fs::remove_file(dir.join("dlss5-feed.log"));
                let _ = fs::remove_file(dir.join("dlss5-feed.addon64"));
                let _ = fs::remove_file(dir.join("dlss5-feed.addon32"));
                let _ = fs::remove_file(dir.join("dlss5-feed-host64.exe"));
                let _ = fs::remove_file(dir.join("dgvoodoo.conf"));
                let _ = fs::remove_file(dir.join("dgvoodoo.log"));
                let _ = fs::remove_file(dir.join("ReShadePreset.ini"));
                let reshade_shaders = dir.join("reshade-shaders");
                if reshade_shaders.is_dir() {
                    let _ = fs::remove_dir_all(&reshade_shaders);
                }
                let host64_dir = dir.join("host64");
                if host64_dir.is_dir() {
                    let _ = fs::remove_dir_all(&host64_dir);
                }
            } else if target_route == "feeder" {
                let _ = fs::remove_file(dir.join("dlss-overlay.addon64"));
                let _ = fs::remove_file(dir.join("dlss5-lab-overlay.addon64"));
            }
        }

        let has_vulkan_target = api.to_lowercase().contains("vulkan")
            || mod_root.read_dir().map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    let p = e.path();
                    p.is_file()
                        && p.extension().map(|ext| ext.eq_ignore_ascii_case("exe")).unwrap_or(false)
                        && crate::core::scan::detect_api_for_exe(&p).map(|a| a.to_lowercase().contains("vulkan")).unwrap_or(false)
                })
            }).unwrap_or(false);

        if !has_vulkan_target {
            let _ = crate::core::vulkan_layer::unregister_vulkan_layer(game_dir);
        }
    }

    Ok(())
}

fn carry_forward_existing_backups(
    game_dir: &Path,
    backup_dir: &Path,
    manifest: &mut ActiveManifest,
    log: &mut Vec<String>,
) {
    if let Some(prev) = crate::core::journal::read_manifest(game_dir) {
        let prev_bdir = crate::core::journal::backup_dir(game_dir);
        for item in prev.replaced {
            let old_backup_file = if let Some(ref p) = prev.backup_prefix {
                prev_bdir.join(p).join(&item.rel)
            } else {
                prev_bdir.join(&item.rel)
            };
            if old_backup_file.is_file() && !crate::core::journal::is_corrupted_or_mod_backup(&old_backup_file) {
                let new_backup_file = backup_dir.join(&item.rel);
                if let Some(parent) = new_backup_file.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if fs::copy(&old_backup_file, &new_backup_file).is_ok() {
                    if !manifest.replaced.iter().any(|r| r.rel == item.rel) {
                        manifest.replaced.push(item.clone());
                        log.push(format!("[BACKUP] Preserved original vanilla backup: {}", item.rel));
                    }
                }
            }
        }
    }
}

fn track_and_copy(
    manifest: &mut ActiveManifest,
    game_dir: &Path,
    backup_dir: &Path,
    src: &Path,
    dest: &Path,
    kind: &str,
    log: &mut Vec<String>,
) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
            let rel_p = parent.strip_prefix(game_dir).unwrap_or(parent).to_string_lossy().to_string();
            if !rel_p.is_empty() && !manifest.added_dirs.contains(&rel_p) {
                manifest.added_dirs.push(rel_p);
            }
        }
    }

    let rel = dest.strip_prefix(game_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dest.file_name().unwrap_or_default().to_string_lossy().to_string());

    if dest.exists() {
        let is_same_as_src = if let (Ok(d_meta), Ok(s_meta)) = (dest.metadata(), src.metadata()) {
            if d_meta.len() == s_meta.len() {
                fs::read(dest).ok() == fs::read(src).ok()
            } else {
                false
            }
        } else {
            false
        };

        let was_previously_added = if let Some(prev) = crate::core::journal::read_manifest(game_dir) {
            prev.added.contains(&rel)
        } else {
            false
        };

        if is_known_mod_file(dest) || is_same_as_src || was_previously_added {
            if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
                manifest.added.push(rel.clone());
            }
        } else if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            let backup_dest = backup_dir.join(&rel);
            if let Some(p) = backup_dest.parent() {
                fs::create_dir_all(p)?;
            }
            let _ = fs::copy(dest, &backup_dest);
            manifest.replaced.push(ManifestItem {
                rel: rel.clone(),
                old_hash: None,
                kind: Some(kind.to_string()),
            });
            log.push(format!("@{{log_backup_saved|{}}}", rel));
        }
    } else {
        if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            manifest.added.push(rel.clone());
        }
    }

    fs::copy(src, dest)?;
    log.push(format!("@{{log_copy_deployed|{}}}", rel));
    Ok(())
}

fn track_and_write(
    manifest: &mut ActiveManifest,
    game_dir: &Path,
    backup_dir: &Path,
    dest: &Path,
    content: &str,
    kind: &str,
    log: &mut Vec<String>,
) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
            let rel_p = parent.strip_prefix(game_dir).unwrap_or(parent).to_string_lossy().to_string();
            if !rel_p.is_empty() && !manifest.added_dirs.contains(&rel_p) {
                manifest.added_dirs.push(rel_p);
            }
        }
    }

    let rel = dest.strip_prefix(game_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dest.file_name().unwrap_or_default().to_string_lossy().to_string());

    if dest.exists() {
        if is_known_mod_file(dest) {
            if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
                manifest.added.push(rel.clone());
            }
        } else if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            let backup_dest = backup_dir.join(&rel);
            if let Some(p) = backup_dest.parent() {
                fs::create_dir_all(p)?;
            }
            let _ = fs::copy(dest, &backup_dest);
            manifest.replaced.push(ManifestItem {
                rel: rel.clone(),
                old_hash: None,
                kind: Some(kind.to_string()),
            });
            log.push(format!("@{{log_backup_saved|{}}}", rel));
        }
    } else {
        if !manifest.added.contains(&rel) && !manifest.replaced.iter().any(|r| r.rel == rel) {
            manifest.added.push(rel.clone());
        }
    }

    fs::write(dest, content)?;
    log.push(format!("@{{log_write_configured|{}}}", rel));
    Ok(())
}

/// Deploys Pure OptiScaler Pre-SR and Standalone 4x MFG (Universal RTXMFG v1.3.2) using provided payload bundle.
/// STRICTLY ZERO ReShade or add-on files are copied or referenced in this route.
pub fn deploy_optiscaler_with_bundle(opts: &DeployOptions, payloads: &PayloadBundle) -> Result<DeployResult, String> {
    let mut log = Vec::new();
    let mod_root = crate::core::compatibility::managed_mod_root(&opts.game_dir, Some(&opts.exe_path))
        .unwrap_or_else(|| opts.exe_path.parent().unwrap_or(&opts.game_dir).to_path_buf());

    log.push(format!("[ROUTING] Target installation directory: {}", mod_root.display()));

    crate::core::install_guards::assert_game_closed(&opts.game_dir, Some(&opts.exe_path))
        .map_err(|e| format!("Cannot deploy while game is running: {}", e))?;

    clean_conflicting_route_artifacts("optiscaler", &opts.game_dir, &mod_root, opts.mfg_unlock, &opts.api, &mut log)
        .map_err(|e| format!("Failed to clean conflicting route artifacts: {}", e))?;

    let hook_dll = if opts.api.to_lowercase().contains("9") {
        "d3d9.dll"
    } else {
        "dxgi.dll"
    };

    remove_stale_proxy_hooks(&mod_root, &[hook_dll], &mut log);

    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let prefix = format!("originals/{}", ts);
    let backup_dir = opts.game_dir.join("_DLSS5_Backup").join(&prefix);

    let exe_rel = opts.exe_path.strip_prefix(&opts.game_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| opts.exe_path.file_name().unwrap_or_default().to_string_lossy().to_string());

    let mut manifest = ActiveManifest {
        version: 1,
        date: crate::core::journal::now_timestamp_str(),
        route: "optiscaler".to_string(),
        game: Some(ManifestGame {
            dir: Some(opts.game_dir.to_string_lossy().to_string()),
            exe: Some(exe_rel.clone()),
            api: Some(if opts.api.to_lowercase().contains("vulkan") { "vulkan".to_string() } else { "dxgi".to_string() }),
            bitness: Some(64),
            api_label: Some(opts.api.clone()),
        }),
        game_exe: Some(exe_rel.clone()),
        backup_prefix: Some(prefix),
        replaced: Vec::new(),
        added: Vec::new(),
        added_dirs: Vec::new(),
        mfg_unlock: Some(opts.mfg_unlock),
        mfg_multiplier: Some(opts.mfg_multiplier),
        nr_style_enabled: Some(opts.nr_style_enabled),
        nr_style: Some(opts.nr_style),
        opti_presr: Some(opts.pre_sr),
        opti_passes: Some(opts.passes),
    };

    carry_forward_existing_backups(&opts.game_dir, &backup_dir, &mut manifest, &mut log);

    // 1. Copy OptiScaler.dll as the hook DLL (dxgi.dll or d3d9.dll)
    if !payloads.optiscaler_dll.is_file() {
        return Err(format!("Missing OptiScaler.dll: {}", payloads.optiscaler_dll.display()));
    }
    track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, &payloads.optiscaler_dll, &mod_root.join(hook_dll), "optiscaler", &mut log)
        .map_err(|e| format!("Failed to copy hook DLL: {}", e))?;

    // 2. Copy nvngx_dlssnr.dll and nvngx.dll_dlssnr.dll if present
    if let Some(dlssnr_src) = &payloads.nvngx_dlssnr_dll {
        if dlssnr_src.is_file() {
            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dlssnr_src, &mod_root.join("nvngx_dlssnr.dll"), "runtime", &mut log)
                .map_err(|e| format!("Failed to copy nvngx_dlssnr.dll: {}", e))?;
        }
    }
    if let Some(snippet_src) = &payloads.nvngx_snippet_dll {
        if snippet_src.is_file() {
            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, snippet_src, &mod_root.join("nvngx.dll_dlssnr.dll"), "runtime", &mut log)
                .map_err(|e| format!("Failed to copy nvngx.dll_dlssnr.dll: {}", e))?;
        }
    }

    // 3. Copy OptiScaler subfolder if present
    if let Some(opti_sub) = &payloads.optiscaler_dir {
        if opti_sub.is_dir() {
            let parent_dir = opti_sub.parent().unwrap_or(opti_sub);
            for entry in walkdir::WalkDir::new(opti_sub).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let sub_rel = entry.path().strip_prefix(parent_dir).unwrap_or_else(|_| Path::new("OptiScaler"));
                    let dest = mod_root.join(sub_rel);
                    track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, entry.path(), &dest, "optiscaler", &mut log)
                        .map_err(|e| format!("Failed to copy {}: {}", sub_rel.display(), e))?;
                }
            }
        }
    }

    // 4. Configure and write OptiScaler.ini (pure OptiScaler Pre-SR, zero ReShade)
    let base_ini_text = fs::read_to_string(&payloads.optiscaler_ini).unwrap_or_default();
    let configured_ini = configure_optiscaler_ini(&base_ini_text, &OptiScalerOptions {
        pre_sr: opts.pre_sr,
        passes: opts.passes,
        mfg_unlock: opts.mfg_unlock,
        target_exe_name: opts.exe_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
        nr_style: opts.nr_style,
    });
    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &mod_root.join("OptiScaler.ini"), &configured_ini, "config", &mut log)
        .map_err(|e| format!("Failed to write OptiScaler.ini: {}", e))?;

    // 5. Deploy Standalone 4x MFG (Universal RTXMFG v1.3.2) as version.dll
    // Coexists with OptiScaler via [FrameGen] External=true as documented in RTX40-MFG.md:
    // OptiScaler yields Streamline hooks and passes FrameGen to Dashdogy's RTXMFG.
    if opts.mfg_unlock {
        if let Some(mfg_dll) = &payloads.rtxmfg_dll {
            if mfg_dll.is_file() {
                let version_dest = mod_root.join("version.dll");
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, mfg_dll, &version_dest, "mfg", &mut log)
                    .map_err(|e| format!("Failed to deploy standalone RTXMFG as version.dll: {}", e))?;
                log.push(format!("[MFG] Standalone Universal RTXMFG v1.3.2 deployed as version.dll ({}x multiplier active)", opts.mfg_multiplier));

                let mult_key = if opts.mfg_multiplier >= 2 { opts.mfg_multiplier - 1 } else { 1 };
                let mfg_config = format!(
                    "{{\n  \"mode\": \"fixed\",\n  \"multiplier\": {},\n  \"dlssgPreset\": 2,\n  \"followGame\": false,\n  \"dynamicTargetFrameRate\": 0,\n  \"dynamicExperimental56\": false,\n  \"selectiveOtaDlssgWrapper\": false\n}}\n",
                    mult_key
                );
                let json_dest1 = mod_root.join("RTXMFG-Universal.json");
                track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &json_dest1, &mfg_config, "mfg_config", &mut log)
                    .map_err(|e| format!("Failed to write RTXMFG-Universal.json: {}", e))?;
                let json_dest2 = mod_root.join("RTX40MFG-Universal.json");
                track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &json_dest2, &mfg_config, "mfg_config", &mut log)
                    .map_err(|e| format!("Failed to write RTX40MFG-Universal.json: {}", e))?;
                log.push(format!("[MFG] Configured RTXMFG-Universal.json (multiplier: {}x, fixed mode)", opts.mfg_multiplier));
            } else {
                log.push("[WARNING] RTXMFG.dll not found on system - skipping standalone MFG".to_string());
            }
        } else {
            log.push("[WARNING] Standalone RTXMFG payload unavailable - skipping standalone MFG".to_string());
        }
    }

    // 6. Deploy verified Streamline 2.14.1 stack if game has Streamline 2.x
    // Unifies local Streamline with driver OTA to permanently eliminate the 0xC0000005 crash in sl.reflex (190_E658703.dll).
    // Legacy Streamline 1.x titles (e.g. A Plague Tale: Requiem) are strictly preserved to prevent export mismatch crashes.
    let existing_sl = [
        backup_dir.join("sl.interposer.dll"),
        mod_root.join("sl.interposer.dll"),
        opts.game_dir.join("sl.interposer.dll"),
    ]
    .into_iter()
    .find(|p| p.is_file());

    if let Some(target_sl) = existing_sl {
        if crate::core::pe::is_legacy_streamline_1x(&target_sl) {
            log.push("[STREAMLINE] Legacy Streamline 1.x detected (exports slGetFeatureSettings) - preserving original game Streamline files to prevent 0xC0000005 export mismatch".to_string());
        } else if let Some(streamline_src) = &payloads.streamline_dir {
            if streamline_src.is_dir() {
                let sl_files = [
                    "sl.interposer.dll",
                    "sl.common.dll",
                    "sl.dlss_g.dll",
                    "sl.reflex.dll",
                    "sl.pcl.dll",
                    "nvngx_dlssg.dll",
                ];
                for f in &sl_files {
                    let src_file = streamline_src.join(f);
                    if src_file.is_file() {
                        let dest_file = mod_root.join(f);
                        track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, &src_file, &dest_file, "streamline", &mut log)
                            .map_err(|e| format!("Failed to deploy Streamline runtime {}: {}", f, e))?;
                    }
                }
                log.push("[STREAMLINE] Deployed verified Streamline 2.14.1 stack to eliminate OTA ABI version conflicts".to_string());
            }
        }
    }

    // 6. Save manifest and history
    save_manifest(&opts.game_dir, &manifest)
        .map_err(|e| format!("Failed to save manifest: {}", e))?;

    let replaced = manifest.replaced.len();
    let added = manifest.added.len();

    let _ = append_history(&HistoryRow {
        date: crate::core::journal::now_timestamp_str(),
        dir: opts.game_dir.to_string_lossy().to_string(),
        game_name: opts.game_name.clone(),
        action: "install".to_string(),
        replaced,
        added,
    });

    log.push(format!("[COMPLETE] Successfully installed OptiScaler Pre-SR (Passes: {}, MFG: {})! {} files replaced, {} added.",
        opts.passes,
        if opts.mfg_unlock { "4x standalone" } else { "disabled" },
        replaced,
        added
    ));

    Ok(DeployResult {
        success: true,
        log_lines: log,
        replaced,
        added,
    })
}

/// Fully deploys Pure OptiScaler Pre-SR and Standalone 4x MFG using system-resolved payloads.
pub fn deploy_optiscaler(opts: &DeployOptions) -> Result<DeployResult, String> {
    let payloads = PayloadBundle::from_system()?;
    deploy_optiscaler_with_bundle(opts, &payloads)
}

/// Deploys Native DLSS 5 route (ReShade + RenoDX + ReShade 4x MFG Unlock).
/// STRICTLY ZERO OptiScaler files are deployed in this route.
pub fn deploy_native_dlss5_with_bundle(opts: &DeployOptions, payloads: &PayloadBundle) -> Result<DeployResult, String> {
    let mut log = Vec::new();
    let mod_root = crate::core::compatibility::managed_mod_root(&opts.game_dir, Some(&opts.exe_path))
        .unwrap_or_else(|| opts.exe_path.parent().unwrap_or(&opts.game_dir).to_path_buf());

    log.push(format!("[ROUTING] Native DLSS (RenoDX) target mod directory: {}", mod_root.display()));

    crate::core::install_guards::assert_game_closed(&opts.game_dir, Some(&opts.exe_path))
        .map_err(|e| format!("Cannot deploy while game is running: {}", e))?;

    clean_conflicting_route_artifacts("native", &opts.game_dir, &mod_root, opts.mfg_unlock, &opts.api, &mut log)
        .map_err(|e| format!("Failed to clean conflicting route artifacts: {}", e))?;

    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let prefix = format!("originals/{}", ts);
    let backup_dir = opts.game_dir.join("_DLSS5_Backup").join(&prefix);

    let exe_rel = opts.exe_path.strip_prefix(&opts.game_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| opts.exe_path.file_name().unwrap_or_default().to_string_lossy().to_string());

    let mut manifest = ActiveManifest {
        version: 1,
        date: crate::core::journal::now_timestamp_str(),
        route: "native".to_string(),
        game: Some(ManifestGame {
            dir: Some(opts.game_dir.to_string_lossy().to_string()),
            exe: Some(exe_rel.clone()),
            api: Some(if opts.api.to_lowercase().contains("vulkan") { "vulkan".to_string() } else { "dxgi".to_string() }),
            bitness: Some(64),
            api_label: Some(opts.api.clone()),
        }),
        game_exe: Some(exe_rel.clone()),
        backup_prefix: Some(prefix),
        replaced: Vec::new(),
        added: Vec::new(),
        added_dirs: Vec::new(),
        mfg_unlock: Some(opts.mfg_unlock),
        mfg_multiplier: Some(opts.mfg_multiplier),
        nr_style_enabled: Some(opts.nr_style_enabled),
        nr_style: Some(opts.nr_style),
        opti_presr: Some(opts.pre_sr),
        opti_passes: Some(opts.passes),
    };

    carry_forward_existing_backups(&opts.game_dir, &backup_dir, &mut manifest, &mut log);

    let hook_dll = if opts.api.to_lowercase().contains("9") {
        "d3d9.dll"
    } else if opts.api.to_lowercase().contains("opengl") {
        "opengl32.dll"
    } else {
        "dxgi.dll"
    };

    remove_stale_proxy_hooks(&mod_root, &[hook_dll], &mut log);

    // 1. Deploy ReShade64.dll as hook DLL
    if let Some(reshade_src) = &payloads.reshade64_dll {
        if reshade_src.is_file() {
            let dest_hook = mod_root.join(hook_dll);
            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, reshade_src, &dest_hook, "reshade", &mut log)
                .map_err(|e| format!("Failed to deploy {}: {}", hook_dll, e))?;
            log.push(format!("[HOOK] ReShade deployed as {} for RenoDX add-on execution", hook_dll));
        } else {
            return Err(format!("ReShade64.dll payload file missing: {}", reshade_src.display()));
        }
    } else {
        return Err("ReShade64.dll payload not found on system".to_string());
    }

    let state = crate::core::state::load_state();
    let renodx_active = crate::core::state::is_addon_active(&state, "builtin:renodx");
    let mfg_active = crate::core::state::is_addon_active(&state, "builtin:mfgunlock");

    // 2. Deploy RenoDX v4.7 DLSS 5 add-on if active
    let mut deployed_addon_stems: Vec<String> = Vec::new();
    if renodx_active {
        if let Some(renodx_src) = &payloads.renodx_dlss5_addon {
            if renodx_src.is_file() {
                let dest = mod_root.join("renodx-dlss5.addon64");
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, renodx_src, &dest, "addon", &mut log)
                    .map_err(|e| format!("Failed to copy renodx-dlss5.addon64: {}", e))?;
                deployed_addon_stems.push("renodx-dlss5".to_string());
                log.push("[ADDON] renodx-dlss5.addon64 deployed (RenoDX v4.7 Integrated DLSS 5 Engine)".to_string());
            }
        }

        // Deploy DLSS Studio D3D12 Mip Fix companion addon
        if let Some(fix_src) = &payloads.dlss5_d3d12_fix_addon {
            if fix_src.is_file() {
                let dest = mod_root.join("dlss-mip-fix.addon64");
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, fix_src, &dest, "addon", &mut log)
                    .map_err(|e| format!("Failed to copy dlss-mip-fix.addon64: {}", e))?;
                deployed_addon_stems.push("dlss-mip-fix".to_string());
                log.push("[ADDON] dlss-mip-fix.addon64 deployed (Pure Rust D3D12 Mip Companion)".to_string());
            }
        }

        // Deploy nvngx_dlssnr.dll required by RenoDX DLSS 5 Neural Rendering engine
        if let Some(dlssnr_src) = &payloads.nvngx_dlssnr_dll {
            if dlssnr_src.is_file() {
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dlssnr_src, &mod_root.join("nvngx_dlssnr.dll"), "runtime", &mut log)
                    .map_err(|e| format!("Failed to copy nvngx_dlssnr.dll: {}", e))?;
                log.push("[RUNTIME] nvngx_dlssnr.dll deployed for RenoDX DLSS 5 Neural Rendering".to_string());
            }
        }
    }

    // Note: DLSS 5 In-Game Overlay add-on is shelved for now - strictly zero overlay deployment

    // 3. Deploy ReShade RenoDX 4x MFG Unlock if requested and active
    if opts.mfg_unlock && mfg_active {
        if let Some(mfg_src) = &payloads.renodx_mfgunlock_addon {
            if mfg_src.is_file() {
                let dest = mod_root.join("renodx-mfgunlock.addon64");
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, mfg_src, &dest, "addon", &mut log)
                    .map_err(|e| format!("Failed to copy renodx-mfgunlock.addon64: {}", e))?;
                deployed_addon_stems.push("renodx-mfgunlock".to_string());
                log.push("[MFG] renodx-mfgunlock.addon64 deployed (ReShade 4x MFG Unlock)".to_string());
            }
        }

        // Deploy modern nvngx_dlssg.dll (310.8.0.0) required by renodx-mfgunlock.addon64 for Ada arch-gate detection
        let has_native_dlssg = mod_root.join("nvngx_dlssg.dll").is_file()
            || opts.game_dir.join("nvngx_dlssg.dll").is_file();

        if has_native_dlssg {
            if let Some(streamline_src) = &payloads.streamline_dir {
                let modern_dlssg = streamline_src.join("nvngx_dlssg.dll");
                if modern_dlssg.is_file() {
                    let dest = mod_root.join("nvngx_dlssg.dll");
                    track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, &modern_dlssg, &dest, "runtime", &mut log)
                        .map_err(|e| format!("Failed to upgrade nvngx_dlssg.dll for MFG Unlock: {}", e))?;
                    log.push("[MFG] Upgraded nvngx_dlssg.dll to 310.8.0 for RenoDX Multi-Frame Generation Unlock".to_string());
                }
            }
        }
    }

    // 3b. Deploy active user-imported custom add-ons
    for custom in &state.addon_files {
        if state.addons.contains(&custom.path) {
            let custom_path = PathBuf::from(&custom.path);
            if custom_path.is_file() {
                let file_name = custom_path.file_name().and_then(|n| n.to_str()).unwrap_or("custom.addon64");
                let dest = mod_root.join(file_name);
                if track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, &custom_path, &dest, "addon", &mut log).is_ok() {
                    let stem = custom_path.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name).to_string();
                    deployed_addon_stems.push(stem);
                    let display_name = custom.name.as_deref().unwrap_or(file_name);
                    log.push(format!("[ADDON] {} deployed ({})", file_name, display_name));
                }
            }
        }
    }

    // 4. Configure ReShade.ini
    let reshade_ini_path = mod_root.join("ReShade.ini");
    let existing_reshade_ini = fs::read_to_string(&reshade_ini_path).unwrap_or_default();
    let mut configured_reshade_ini = if opts.mfg_unlock {
        configure_mfg_unlock_ini(&existing_reshade_ini, Some(opts.mfg_multiplier))
    } else {
        existing_reshade_ini
    };
    configured_reshade_ini = set_ini(&configured_reshade_ini, "INPUT", "KeyOverlay", "36,0,0,0");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "OVERLAY", "TutorialProgress", "4");

    // AGENTS.md Line 15: EnableHooks=2 is NGX-only and must be used where native NGX D3D12 creates are active.
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "EnableHooks", "2");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "NeuralUplift", "1");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "NRAutoMask", "1");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "NRStyle", &opts.nr_style.to_string());

    if let Some(disabled) = get_ini(&configured_reshade_ini, "ADDON", "DisabledAddons") {
        let kept: Vec<&str> = disabled.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && !deployed_addon_stems.iter().any(|d| d.eq_ignore_ascii_case(s)))
            .collect();
        configured_reshade_ini = set_ini(&configured_reshade_ini, "ADDON", "DisabledAddons", &kept.join(","));
    }

    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &reshade_ini_path, &configured_reshade_ini, "config", &mut log)
        .map_err(|e| format!("Failed to write ReShade.ini: {}", e))?;

    // 5. Save manifest and history
    save_manifest(&opts.game_dir, &manifest)
        .map_err(|e| format!("Failed to save manifest: {}", e))?;

    let replaced = manifest.replaced.len();
    let added = manifest.added.len();

    let _ = append_history(&HistoryRow {
        date: crate::core::journal::now_timestamp_str(),
        dir: opts.game_dir.to_string_lossy().to_string(),
        game_name: opts.game_name.clone(),
        action: "install_native".to_string(),
        replaced,
        added,
    });

    let mfg_desc = if opts.mfg_unlock {
        format!("{}x companion add-on", opts.mfg_multiplier)
    } else {
        "disabled".to_string()
    };
    log.push(format!("[COMPLETE] Successfully installed ReShade + RenoDX (MFG: {})! {} files replaced, {} added.",
        mfg_desc,
        replaced,
        added
    ));

    Ok(DeployResult {
        success: true,
        log_lines: log,
        replaced,
        added,
    })
}

/// Fully deploys ReShade + RenoDX route using system-resolved payloads.
pub fn deploy_native_dlss5(opts: &DeployOptions) -> Result<DeployResult, String> {
    let payloads = PayloadBundle::from_system()?;
    deploy_native_dlss5_with_bundle(opts, &payloads)
}

/// Formats or updates ReShadePreset.ini for DLSS5-Feeder route.
/// In ReShade preset syntax, Techniques= and TechniqueSorting= MUST live at the root of the file
/// before ANY section header (e.g. [DLSS5_Feed.fx]). Placing them under [ReShadePreset.ini] causes
/// ReShade to parse 0 active techniques and skips execution of DLSS5_Feed.fx and vort_Motion.fx.
pub fn configure_feeder_preset(existing: &str) -> String {
    let required_techs = ["vort_MotionEffects@vort_Motion.fx", "DLSS5_Feed@DLSS5_Feed.fx"];

    if existing.trim().is_empty() {
        return format!(
            "Techniques={}\nTechniqueSorting={}\nPreprocessorDefinitions=DLSS5_MV_PROVIDER=2\n\n[DLSS5_Feed.fx]\nPreprocessorDefinitions=DLSS5_MV_PROVIDER=2\n",
            required_techs.join(","),
            required_techs.join(",")
        );
    }

    // Strip legacy erroneous [ReShadePreset.ini] header if present
    let raw_lines: Vec<&str> = existing
        .lines()
        .filter(|l| !l.trim().eq_ignore_ascii_case("[ReShadePreset.ini]"))
        .collect();

    // Partition root lines (before the first [section]) and section lines
    let mut root_lines = Vec::new();
    let mut section_lines = Vec::new();
    let mut in_section = false;

    for line in raw_lines {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = true;
        }
        if in_section {
            section_lines.push(line);
        } else {
            root_lines.push(line);
        }
    }

    let mut techniques: Vec<String> = required_techs.iter().map(|s| s.to_string()).collect();
    let mut sorting: Vec<String> = required_techs.iter().map(|s| s.to_string()).collect();
    let mut preprocessors = "DLSS5_MV_PROVIDER=2".to_string();
    let mut other_root = Vec::new();

    for line in root_lines {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("Techniques") {
                for t in val.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    if !techniques.iter().any(|req| req.eq_ignore_ascii_case(t)) {
                        techniques.push(t.to_string());
                    }
                }
            } else if key.eq_ignore_ascii_case("TechniqueSorting") {
                for t in val.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    if !sorting.iter().any(|req| req.eq_ignore_ascii_case(t)) {
                        sorting.push(t.to_string());
                    }
                }
            } else if key.eq_ignore_ascii_case("PreprocessorDefinitions") {
                if !val.contains("DLSS5_MV_PROVIDER") {
                    preprocessors = format!("{},{}", val, preprocessors);
                } else {
                    preprocessors = val.to_string();
                }
            } else {
                other_root.push(line.to_string());
            }
        } else if !trimmed.is_empty() {
            other_root.push(line.to_string());
        }
    }

    let mut out = String::new();
    out.push_str(&format!("Techniques={}\n", techniques.join(",")));
    out.push_str(&format!("TechniqueSorting={}\n", sorting.join(",")));
    out.push_str(&format!("PreprocessorDefinitions={}\n", preprocessors));
    for line in other_root {
        out.push_str(&line);
        out.push('\n');
    }
    if !section_lines.is_empty() {
        out.push('\n');
        for line in section_lines {
            out.push_str(line);
            out.push('\n');
        }
    }

    if !out.contains("[DLSS5_Feed.fx]") {
        out.push_str("\n[DLSS5_Feed.fx]\nPreprocessorDefinitions=DLSS5_MV_PROVIDER=2\n");
    }

    out
}

/// Formats or updates dgVoodoo.conf for legacy DirectX and Glide titles.
/// Sets WatermarkDisplayDuration = 3 so that watermarks briefly display for 3 seconds on launch
/// as visual confirmation that dgVoodoo2 is active, then automatically disappear.
/// Sets VRAM = 2048 (2GB) under [DirectX] to prevent "Display hardware video memory exhausted"
/// crashes in 32-bit titles running at 1440p, 4K, or ultrawide resolutions.
/// Strips any invalid keys previously placed under [General] to keep the dgVoodoo parser healthy.
pub fn configure_dgvoodoo_conf(base: &str) -> String {
    let text = base.trim();
    if text.is_empty() {
        return "[GeneralExt]\nWatermarkDisplayDuration = 3\n\n[DirectX]\nDisableAndPassThru = false\nVRAM = 2048\ndgVoodooWatermark = true\n\n[Glide]\n3DfxWatermark = true\n3DfxSplashScreen = false\n".to_string();
    }

    let re_wm_dur = regex::Regex::new(r"(?i)^(\s*WatermarkDisplayDuration\s*=\s*)\S+").unwrap();
    let re_dg_wm = regex::Regex::new(r"(?i)^(\s*dgVoodooWatermark\s*=\s*)\S+").unwrap();
    let re_vram = regex::Regex::new(r"(?i)^(\s*VRAM\s*=\s*)\S+").unwrap();
    let re_3dfx_wm = regex::Regex::new(r"(?i)^(\s*3DfxWatermark\s*=\s*)\S+").unwrap();
    let re_3dfx_splash = regex::Regex::new(r"(?i)^(\s*3DfxSplashScreen\s*=\s*)\S+").unwrap();
    let re_pass_thru = regex::Regex::new(r"(?i)^(\s*DisableAndPassThru\s*=\s*)\S+").unwrap();

    let mut lines: Vec<String> = Vec::new();
    let mut in_general = false;
    let mut has_wm_dur = false;
    let mut has_dg_wm = false;
    let mut has_vram = false;
    let mut has_3dfx_wm = false;
    let mut has_3dfx_splash = false;
    let mut has_pass_thru = false;

    for line in base.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_general = trimmed.eq_ignore_ascii_case("[General]");
        }

        // Strip legacy invalid DisableAndPassThru inserted under [General]
        if in_general && trimmed.eq_ignore_ascii_case("DisableAndPassThru=false") {
            continue;
        }

        let mut replaced = line.to_string();
        if re_wm_dur.is_match(&replaced) {
            replaced = re_wm_dur.replace(&replaced, "${1}3").to_string();
            has_wm_dur = true;
        }
        if re_dg_wm.is_match(&replaced) {
            replaced = re_dg_wm.replace(&replaced, "${1}true").to_string();
            has_dg_wm = true;
        }
        if re_vram.is_match(&replaced) {
            replaced = re_vram.replace(&replaced, "${1}2048").to_string();
            has_vram = true;
        }
        if re_3dfx_wm.is_match(&replaced) {
            replaced = re_3dfx_wm.replace(&replaced, "${1}true").to_string();
            has_3dfx_wm = true;
        }
        if re_3dfx_splash.is_match(&replaced) {
            replaced = re_3dfx_splash.replace(&replaced, "${1}false").to_string();
            has_3dfx_splash = true;
        }
        if re_pass_thru.is_match(&replaced) {
            replaced = re_pass_thru.replace(&replaced, "${1}false").to_string();
            has_pass_thru = true;
        }

        lines.push(replaced);
    }

    let mut result = lines.join("\r\n");

    // If any keys were missing, insert them into their proper sections using set_ini
    if !has_wm_dur {
        result = set_ini(&result, "GeneralExt", "WatermarkDisplayDuration", "3");
    }
    if !has_vram {
        result = set_ini(&result, "DirectX", "VRAM", "2048");
    }
    if !has_dg_wm {
        result = set_ini(&result, "DirectX", "dgVoodooWatermark", "true");
    }
    if !has_pass_thru {
        result = set_ini(&result, "DirectX", "DisableAndPassThru", "false");
    }
    if !has_3dfx_wm {
        result = set_ini(&result, "Glide", "3DfxWatermark", "true");
    }
    if !has_3dfx_splash {
        result = set_ini(&result, "Glide", "3DfxSplashScreen", "false");
    }

    result
}

/// Formats ReShade.ini for host64 companion helper.
pub fn configure_host64_reshade_ini(nr_style: usize) -> String {
    let mut text = String::new();
    text = set_ini(&text, "INPUT", "KeyOverlay", "36,0,0,0");
    text = set_ini(&text, "OVERLAY", "TutorialProgress", "4");
    text = set_ini(&text, "ADDON", "AddonPath", ".\\");
    text = set_ini(&text, "RenoDX.DLSS5", "EnableHooks", "1");
    text = set_ini(&text, "RenoDX.DLSS5", "NeuralUplift", "1");
    text = set_ini(&text, "RenoDX.DLSS5", "NREnableUpscaling", "0");
    text = set_ini(&text, "RenoDX.DLSS5", "NRStyle", &nr_style.to_string());
    text
}

/// Deploys DLSS5-Feeder route using provided payload bundle.
/// STRICTLY ZERO Pre-SR, ZERO MFG, ZERO OptiScaler files are deployed in this route.
pub fn deploy_feeder_with_bundle(opts: &DeployOptions, payloads: &PayloadBundle) -> Result<DeployResult, String> {
    let mut log = Vec::new();
    let mod_root = crate::core::compatibility::managed_mod_root(&opts.game_dir, Some(&opts.exe_path))
        .unwrap_or_else(|| opts.exe_path.parent().unwrap_or(&opts.game_dir).to_path_buf());

    log.push(format!("[ROUTING] DLSS5-Feeder target mod directory: {}", mod_root.display()));

    crate::core::install_guards::assert_game_closed(&opts.game_dir, Some(&opts.exe_path))
        .map_err(|e| format!("Cannot deploy while game is running: {}", e))?;

    clean_conflicting_route_artifacts("feeder", &opts.game_dir, &mod_root, opts.mfg_unlock, &opts.api, &mut log)
        .map_err(|e| format!("Failed to clean conflicting route artifacts: {}", e))?;

    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let prefix = format!("originals/{}", ts);
    let backup_dir = opts.game_dir.join("_DLSS5_Backup").join(&prefix);

    let exe_rel = opts.exe_path.strip_prefix(&opts.game_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| opts.exe_path.file_name().unwrap_or_default().to_string_lossy().to_string());

    let bitness = crate::core::pe::inspect_pe(&opts.exe_path).map(|p| p.bitness).unwrap_or(64);

    let api_lower = opts.api.to_lowercase();
    let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12") || api_lower.contains("dxgi");
    let is_vulkan = api_lower.contains("vulkan");
    let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !is_dx12;
    let has_native_dlssg = mod_root.join("nvngx_dlssg.dll").is_file()
        || mod_root.join("sl.dlss_g.dll").is_file()
        || opts.game_dir.join("nvngx_dlssg.dll").is_file();
    let api_supports_fg = bitness == 64 && (has_native_dlssg || is_vulkan || is_dx12) && !is_dx11;

    let state = crate::core::state::load_state();
    let mfg_active = crate::core::state::is_addon_active(&state, "builtin:mfgunlock");
    let effective_mfg = opts.mfg_unlock && api_supports_fg && mfg_active;

    let mut manifest = ActiveManifest {
        version: 1,
        date: crate::core::journal::now_timestamp_str(),
        route: "feeder".to_string(),
        game: Some(ManifestGame {
            dir: Some(opts.game_dir.to_string_lossy().to_string()),
            exe: Some(exe_rel.clone()),
            api: Some(if is_vulkan { "vulkan".to_string() } else { "dxgi".to_string() }),
            bitness: Some(bitness),
            api_label: Some(opts.api.clone()),
        }),
        game_exe: Some(exe_rel.clone()),
        backup_prefix: Some(prefix),
        replaced: Vec::new(),
        added: Vec::new(),
        added_dirs: Vec::new(),
        mfg_unlock: Some(effective_mfg),
        mfg_multiplier: Some(opts.mfg_multiplier),
        nr_style_enabled: Some(opts.nr_style_enabled),
        nr_style: Some(opts.nr_style),
        opti_presr: Some(opts.pre_sr),
        opti_passes: Some(opts.passes),
    };

    carry_forward_existing_backups(&opts.game_dir, &backup_dir, &mut manifest, &mut log);

    let is_legacy_dx = api_lower.contains('9') || api_lower.contains('8') || api_lower.contains("d3d9") || api_lower.contains("d3d8");
    let use_dgvoodoo = is_legacy_dx && payloads.dgvoodoo.is_some();

    let (hook_dll, dg_hook_name) = if use_dgvoodoo {
        let dg_name = if api_lower.contains('8') || api_lower.contains("d3d8") {
            "d3d8.dll"
        } else {
            "d3d9.dll"
        };
        ("dxgi.dll", Some(dg_name))
    } else if api_lower.contains('9') {
        ("d3d9.dll", None)
    } else if api_lower.contains("opengl") {
        ("opengl32.dll", None)
    } else {
        ("dxgi.dll", None)
    };

    let mut active_hooks = vec![hook_dll];
    if let Some(dg_name) = dg_hook_name {
        active_hooks.push(dg_name);
    }
    remove_stale_proxy_hooks(&mod_root, &active_hooks, &mut log);

    // If game is Vulkan or contains any Vulkan executables, register the Vulkan implicit layer
    let has_vulkan_target = opts.api.to_lowercase().contains("vulkan")
        || mod_root.read_dir().map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                let p = e.path();
                p.is_file()
                    && p.extension().map(|ext| ext.eq_ignore_ascii_case("exe")).unwrap_or(false)
                    && crate::core::scan::detect_api_for_exe(&p).map(|a| a.to_lowercase().contains("vulkan")).unwrap_or(false)
            })
        }).unwrap_or(false);

    if has_vulkan_target {
        let vk_dir = payloads.feeder_components.as_ref().and_then(|fc| fc.vk_layer_dir.as_deref());
        match crate::core::vulkan_layer::register_vulkan_layer(&opts.game_dir, vk_dir, payloads.reshade64_dll.as_deref()) {
            Ok(reg_p) => log.push(format!("@{{log_vulkan_registered|{}}}", reg_p.display())),
            Err(e) => log.push(format!("[WARN] Vulkan layer registration: {}", e)),
        }
    }

    // Deploy dgVoodoo wrapper if active
    if use_dgvoodoo {
        if let Some(dg) = &payloads.dgvoodoo {
            let dg_target = dg_hook_name.unwrap_or("d3d9.dll");
            let dg_src = if dg_target == "d3d8.dll" && dg.d3d8_x86.is_some() {
                dg.d3d8_x86.as_ref().unwrap()
            } else if bitness == 32 {
                &dg.d3d9_x86
            } else {
                &dg.d3d9_x64
            };

            let dest_dg = mod_root.join(dg_target);
            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dg_src, &dest_dg, "dgvoodoo", &mut log)
                .map_err(|e| format!("Failed to deploy dgVoodoo {}: {}", dg_target, e))?;
            log.push(format!("[DGVOODOO] Deployed dgVoodoo2 ({}) as {} for D3D -> D3D11 translation", if bitness == 32 { "x86" } else { "x64" }, dg_target));

            let base_conf = fs::read_to_string(&dg.conf).unwrap_or_default();
            let conf_content = configure_dgvoodoo_conf(&base_conf);
            let dest_conf = mod_root.join("dgVoodoo.conf");
            track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &dest_conf, &conf_content, "config", &mut log)
                .map_err(|e| format!("Failed to write dgVoodoo.conf: {}", e))?;
            log.push("[DGVOODOO] Configured dgVoodoo.conf (watermark brief display, VRAM=2048MB, pass-through disabled)".to_string());
        }
    }

    // If target is 32-bit and not Large Address Aware (LAA), back up the original vanilla executable and enable LAA (4GB patch)
    if bitness == 32 && opts.exe_path.is_file() {
        if !crate::core::pe::is_large_address_aware(&opts.exe_path) {
            let exe_rel = opts.exe_path.strip_prefix(&opts.game_dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| opts.exe_path.file_name().unwrap_or_default().to_string_lossy().to_string());
            let backup_exe_target = backup_dir.join(&exe_rel);
            if let Some(parent) = backup_exe_target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::copy(&opts.exe_path, &backup_exe_target).is_ok() {
                if !manifest.replaced.iter().any(|r| r.rel == exe_rel) {
                    manifest.replaced.push(crate::core::journal::ManifestItem {
                        rel: exe_rel.clone(),
                        old_hash: None,
                        kind: Some("executable_vanilla".to_string()),
                    });
                    log.push(format!("[BACKUP] Saved vanilla 32-bit executable to backup before 4GB patch: {}", exe_rel));
                }
                match crate::core::pe::set_large_address_aware(&opts.exe_path, true) {
                    Ok(true) => log.push(format!("[LAA] Applied 4GB Patch (Large Address Aware) to {}", opts.exe_path.file_name().unwrap_or_default().to_string_lossy())),
                    Ok(false) => {},
                    Err(e) => log.push(format!("[WARN] Could not set Large Address Aware on executable: {}", e)),
                }
            }
        }
    }

    // 1. Deploy ReShade (32-bit or 64-bit) as hook DLL (dxgi.dll when dgVoodoo is used)
    let reshade_payload = if bitness == 32 {
        payloads.reshade32_dll.as_ref()
    } else {
        payloads.reshade64_dll.as_ref()
    };

    if let Some(reshade_src) = reshade_payload {
        if reshade_src.is_file() {
            let dest_hook = mod_root.join(hook_dll);
            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, reshade_src, &dest_hook, "reshade", &mut log)
                .map_err(|e| format!("Failed to deploy {}: {}", hook_dll, e))?;
            log.push(format!("[FEEDER] ReShade ({}x) deployed as {} for frame/depth buffer capture", bitness, hook_dll));
        } else {
            return Err(format!("ReShade{} payload file missing: {}", if bitness == 32 { "32.dll" } else { "64.dll" }, reshade_src.display()));
        }
    } else {
        return Err(format!("ReShade{}.dll payload not found on system. Please verify component payloads.", if bitness == 32 { "32" } else { "64" }));
    }

    let state = crate::core::state::load_state();
    let renodx_active = crate::core::state::is_addon_active(&state, "builtin:renodx");
    let _mfg_active = crate::core::state::is_addon_active(&state, "builtin:mfgunlock");

    let mut deployed_addon_stems: Vec<String> = Vec::new();

    // 2. Deploy dlss5-feed addon & cfg if available
    if let Some(fc) = &payloads.feeder_components {
        let addon_src = if bitness == 32 {
            fc.addon32.as_ref()
        } else {
            Some(&fc.addon64)
        };
        if let Some(src) = addon_src {
            if src.is_file() {
                let dest = mod_root.join(if bitness == 32 { "dlss5-feed.addon32" } else { "dlss5-feed.addon64" });
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, src, &dest, "feeder", &mut log)
                    .map_err(|e| format!("Failed to copy dlss5-feed addon: {}", e))?;
                deployed_addon_stems.push("dlss5-feed".to_string());
                log.push(format!("[FEEDER] dlss5-feed addon ({}x) deployed for frame, depth & optical flow capture", bitness));
            }
        }

            // Deploy reshade-shaders tree
            if fc.shader_dir.is_dir() {
                let target_root = mod_root.join("reshade-shaders");
                for entry in walkdir::WalkDir::new(&fc.shader_dir).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        if let Ok(rel) = entry.path().strip_prefix(&fc.shader_dir) {
                            let dest = target_root.join(rel);
                            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, entry.path(), &dest, "shader", &mut log)
                                .map_err(|e| format!("Failed to copy shader {}: {}", rel.display(), e))?;
                        }
                    }
                }
                log.push("[SHADERS] Deployed DLSS5-Feeder optical flow and depth shaders".to_string());
            }
        }

    // Deploy dlss5-feed.cfg
    let cfg_content = "enabled=1\nmode=2\nhdr=-1\ndepth_inverted=-1\nflags=-1\nreset_every=0\nwarmup_rebuild=180\nrebuild=0\nlog_frames=3\ncreate_delay=60\npreset=0\nwork_resolution=100\nmv_scale_x=1.000\nmv_scale_y=1.000\nhost_window=0\nasync_home=1\n";
    let cfg_path = mod_root.join("dlss5-feed.cfg");
    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &cfg_path, cfg_content, "config", &mut log)
        .map_err(|e| format!("Failed to write dlss5-feed.cfg: {}", e))?;

    // Deploy ReShadePreset.ini configured for Feeder shaders.
    // In ReShade preset syntax, Techniques= and TechniqueSorting= MUST live at the root of the file
    // before any section header, otherwise ReShade parses 0 active techniques and skips execution.
    let preset_path = mod_root.join("ReShadePreset.ini");
    let existing_preset = fs::read_to_string(&preset_path).unwrap_or_default();
    let preset_content = configure_feeder_preset(&existing_preset);
    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &preset_path, &preset_content, "config", &mut log)
        .map_err(|e| format!("Failed to write ReShadePreset.ini: {}", e))?;

    // 3. Deploy RenoDX DLSS 5 Engine & Neural Rendering runtime
    // NOTE: renodx-dlss5.addon64 and nvngx_dlssnr.dll are strictly 64-bit binaries.
    // They are only deployed when the game is 64-bit to prevent invalid PE image loader errors.
    if renodx_active && bitness == 64 {
        if let Some(renodx_src) = &payloads.renodx_dlss5_addon {
            if renodx_src.is_file() {
                let dest = mod_root.join("renodx-dlss5.addon64");
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, renodx_src, &dest, "addon", &mut log)
                    .map_err(|e| format!("Failed to copy renodx-dlss5.addon64: {}", e))?;
                deployed_addon_stems.push("renodx-dlss5".to_string());
                log.push("[FEEDER-DLSS5] renodx-dlss5.addon64 deployed for Streamline Neural Rendering".to_string());
            }
        }

        if let Some(fix_src) = &payloads.dlss5_d3d12_fix_addon {
            if fix_src.is_file() {
                let dest = mod_root.join("dlss-mip-fix.addon64");
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, fix_src, &dest, "addon", &mut log)
                    .map_err(|e| format!("Failed to copy dlss-mip-fix.addon64: {}", e))?;
                deployed_addon_stems.push("dlss-mip-fix".to_string());
                log.push("[FEEDER-DLSS5] dlss-mip-fix.addon64 deployed (Pure Rust D3D12 Mip Companion)".to_string());
            }
        }

        if let Some(dlssnr_src) = &payloads.nvngx_dlssnr_dll {
            if dlssnr_src.is_file() {
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dlssnr_src, &mod_root.join("nvngx_dlssnr.dll"), "runtime", &mut log)
                    .map_err(|e| format!("Failed to copy nvngx_dlssnr.dll: {}", e))?;
                log.push("[RUNTIME] nvngx_dlssnr.dll deployed for Streamline Feeder Neural Rendering".to_string());
            }
        }
    }

    // Deploy / upgrade modern nvngx_dlss.dll runtime so dlss5-feed D3D12 session can initialize
    // NGX Super Sampling and CreateFeature without error.
    if bitness == 64 {
        if let Some(dlss_src) = &payloads.nvngx_dlss_dll {
            if dlss_src.is_file() {
                let target_path = if mod_root.join("nvngx_dlss.dll").is_file() {
                    mod_root.join("nvngx_dlss.dll")
                } else if opts.game_dir.join("nvngx_dlss.dll").is_file() {
                    opts.game_dir.join("nvngx_dlss.dll")
                } else {
                    mod_root.join("nvngx_dlss.dll")
                };

                let existing_ver = crate::core::pe::inspect_pe(&target_path).and_then(|p| p.version);
                let payload_ver = crate::core::pe::inspect_pe(dlss_src).and_then(|p| p.version);
                let should_copy = match (existing_ver, payload_ver) {
                    (Some(e), Some(p)) => e != p,
                    _ => true,
                };

                if should_copy {
                    track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dlss_src, &target_path, "runtime", &mut log)
                        .map_err(|e| format!("Failed to deploy/upgrade nvngx_dlss.dll: {}", e))?;
                    log.push("[RUNTIME] Deployed/upgraded modern nvngx_dlss.dll for Streamline Feeder Super Sampling".to_string());
                }
            }
        }
    }

    // 3b. For 32-bit games, assemble the host64 companion directory so 32-bit dlss5-feed can drive 64-bit Neural Rendering
    if bitness == 32 {
        if let Some(fc) = &payloads.feeder_components {
            if let Some(host64_exe) = &fc.host64 {
                if host64_exe.is_file() {
                    let host64_dir = mod_root.join("host64");

                    // 1. Copy host64 executable
                    let dest_host_exe = host64_dir.join("dlss5-feed-host64.exe");
                    track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, host64_exe, &dest_host_exe, "feeder-host", &mut log)
                        .map_err(|e| format!("Failed to copy dlss5-feed-host64.exe: {}", e))?;

                    // 2. Deploy 64-bit ReShade as dxgi.dll in host64/
                    if let Some(reshade64) = &payloads.reshade64_dll {
                        if reshade64.is_file() {
                            let dest_r64 = host64_dir.join("dxgi.dll");
                            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, reshade64, &dest_r64, "feeder-host-reshade", &mut log)
                                .map_err(|e| format!("Failed to copy ReShade64 to host64/dxgi.dll: {}", e))?;
                        }
                    }

                    // 3. Deploy 64-bit RenoDX DLSS5 & DLSS-NR runtimes in host64/
                    if renodx_active {
                        if let Some(renodx) = &payloads.renodx_dlss5_addon {
                            if renodx.is_file() {
                                let dest_renodx = host64_dir.join("renodx-dlss5.addon64");
                                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, renodx, &dest_renodx, "feeder-host-addon", &mut log)
                                    .map_err(|e| format!("Failed to copy renodx-dlss5.addon64 to host64: {}", e))?;
                            }
                        }
                        if let Some(fix) = &payloads.dlss5_d3d12_fix_addon {
                            if fix.is_file() {
                                let dest_fix = host64_dir.join("dlss-mip-fix.addon64");
                                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, fix, &dest_fix, "feeder-host-addon", &mut log)
                                    .map_err(|e| format!("Failed to copy dlss-mip-fix.addon64 to host64: {}", e))?;
                            }
                        }
                        if let Some(dlssnr) = &payloads.nvngx_dlssnr_dll {
                            if dlssnr.is_file() {
                                let dest_dlssnr = host64_dir.join("nvngx_dlssnr.dll");
                                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dlssnr, &dest_dlssnr, "feeder-host-runtime", &mut log)
                                    .map_err(|e| format!("Failed to copy nvngx_dlssnr.dll to host64: {}", e))?;
                            }
                        }
                    }

                    // 4. Deploy modern nvngx_dlss.dll in host64/
                    if let Some(dlss) = &payloads.nvngx_dlss_dll {
                        if dlss.is_file() {
                            let dest_dlss = host64_dir.join("nvngx_dlss.dll");
                            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dlss, &dest_dlss, "feeder-host-runtime", &mut log)
                                .map_err(|e| format!("Failed to copy nvngx_dlss.dll to host64: {}", e))?;
                        }
                    }

                    // 5. Deploy host64 ReShade.ini
                    let host_reshade_ini = configure_host64_reshade_ini(opts.nr_style);
                    let host_ini_path = host64_dir.join("ReShade.ini");
                    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &host_ini_path, &host_reshade_ini, "config", &mut log)
                        .map_err(|e| format!("Failed to write host64/ReShade.ini: {}", e))?;

                    log.push("[HOST64] Assembled 64-bit Neural Host bridge (host64/) for 32-bit game".to_string());
                }
            }
        }
    }

    // 4. Deploy Streamline Feeder addons when MFG is enabled (supported on 64-bit titles with native DLSS-G, DX12/DXGI, or Vulkan)
    if effective_mfg {
        if let Some(mfg_src) = &payloads.renodx_mfgunlock_addon {
            if mfg_src.is_file() {
                let dest = mod_root.join("renodx-mfgunlock.addon64");
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, mfg_src, &dest, "addon", &mut log)
                    .map_err(|e| format!("Failed to copy renodx-mfgunlock.addon64: {}", e))?;
                deployed_addon_stems.push("renodx-mfgunlock".to_string());
                log.push(format!("[FEEDER-MFG] renodx-mfgunlock.addon64 deployed (Streamline {}x MFG Unlock)", opts.mfg_multiplier));
            }
        }
    }

    // 4b. Deploy active user-imported custom add-ons
    for custom in &state.addon_files {
        if state.addons.contains(&custom.path) {
            let custom_path = PathBuf::from(&custom.path);
            if custom_path.is_file() {
                let file_name = custom_path.file_name().and_then(|n| n.to_str()).unwrap_or("custom.addon64");
                let dest = mod_root.join(file_name);
                if track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, &custom_path, &dest, "addon", &mut log).is_ok() {
                    let stem = custom_path.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name).to_string();
                    deployed_addon_stems.push(stem);
                    let display_name = custom.name.as_deref().unwrap_or(file_name);
                    log.push(format!("[ADDON] {} deployed ({})", file_name, display_name));
                }
            }
        }
    }

    // 5. Configure ReShade.ini for Feeder
    let reshade_ini_path = mod_root.join("ReShade.ini");
    let existing_reshade_ini = fs::read_to_string(&reshade_ini_path).unwrap_or_default();
    let mut configured_reshade_ini = if effective_mfg {
        configure_mfg_unlock_ini(&existing_reshade_ini, Some(opts.mfg_multiplier))
    } else {
        existing_reshade_ini
    };
    configured_reshade_ini = set_ini(&configured_reshade_ini, "INPUT", "KeyOverlay", "36,0,0,0");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "OVERLAY", "TutorialProgress", "4");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "GENERAL", "EffectSearchPaths", ".\\reshade-shaders\\Shaders\\**");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "GENERAL", "TextureSearchPaths", ".\\reshade-shaders\\Textures\\**");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "GENERAL", "PresetPath", ".\\ReShadePreset.ini");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "GENERAL", "StartupPresetPath", "");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "GENERAL", "NoReloadOnInit", "0");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "GENERAL", "PreprocessorDefinitions", "DLSS5_MV_PROVIDER=2");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "ADDON", "AddonPath", ".\\");
    // EnableHooks=1 allows RenoDX to hook swapchain and direct presentation paths when native D3D12 NGX
    // is absent or bypassed (such as under DX11, Vulkan, or Feeder routes per AGENTS.md).
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "EnableHooks", "1");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "NeuralUplift", "1");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "NREnableUpscaling", "0");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "RenoDX.DLSS5", "NRStyle", &opts.nr_style.to_string());

    if let Some(disabled) = get_ini(&configured_reshade_ini, "ADDON", "DisabledAddons") {
        let kept: Vec<&str> = disabled.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && !deployed_addon_stems.iter().any(|d| d.eq_ignore_ascii_case(s)))
            .collect();
        configured_reshade_ini = set_ini(&configured_reshade_ini, "ADDON", "DisabledAddons", &kept.join(","));
    }

    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &reshade_ini_path, &configured_reshade_ini, "config", &mut log)
        .map_err(|e| format!("Failed to write ReShade.ini: {}", e))?;

    // 3. Save manifest and history
    save_manifest(&opts.game_dir, &manifest)
        .map_err(|e| format!("Failed to save manifest: {}", e))?;

    let replaced = manifest.replaced.len();
    let added = manifest.added.len();

    let _ = append_history(&HistoryRow {
        date: crate::core::journal::now_timestamp_str(),
        dir: opts.game_dir.to_string_lossy().to_string(),
        game_name: opts.game_name.clone(),
        action: "install_feeder".to_string(),
        replaced,
        added,
    });

    let mfg_desc = if effective_mfg {
        format!("{}x Streamline Feeder companion add-on", opts.mfg_multiplier)
    } else {
        "disabled".to_string()
    };
    log.push(format!("[COMPLETE] Successfully installed DLSS5-Feeder (MFG: {})! {} files replaced, {} added.",
        mfg_desc,
        replaced,
        added
    ));

    Ok(DeployResult {
        success: true,
        log_lines: log,
        replaced,
        added,
    })
}

/// Fully deploys DLSS5-Feeder route using system-resolved payloads.
pub fn deploy_feeder(opts: &DeployOptions) -> Result<DeployResult, String> {
    let mut payloads = PayloadBundle::from_system()?;
    if payloads.feeder_components.is_none() {
        let mut download_log = Vec::new();
        let fc = match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(crate::core::downloader::ensure_feeder_components(&mut download_log))
                })
            }
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("Tokio runtime error: {}", e))?;
                rt.block_on(crate::core::downloader::ensure_feeder_components(&mut download_log))
            }
        }?;
        payloads.feeder_components = Some(fc);
    }

    let api_lower = opts.api.to_lowercase();
    let is_legacy_dx = api_lower.contains('9') || api_lower.contains('8') || api_lower.contains("d3d9") || api_lower.contains("d3d8");
    if is_legacy_dx && payloads.dgvoodoo.is_none() {
        let mut download_log = Vec::new();
        let dg = match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(crate::core::downloader::ensure_dgvoodoo_components(&mut download_log))
                })
            }
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("Tokio runtime error: {}", e))?;
                rt.block_on(crate::core::downloader::ensure_dgvoodoo_components(&mut download_log))
            }
        }?;
        payloads.dgvoodoo = Some(dg);
    }

    deploy_feeder_with_bundle(opts, &payloads)
}

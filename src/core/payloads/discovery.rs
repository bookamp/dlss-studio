//! Runtime component payload discovery functions across system locations.

use std::fs;
use std::path::{Path, PathBuf};

use super::scoring::score_optiscaler_dir;

/// Locates the application executable's parent directory.
pub fn app_exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// Returns standard candidate directories to check for runtime component payloads.
pub fn get_component_roots() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(crate::core::addons::get_components_root());
    if let Some(exe) = app_exe_dir() {
        dirs.push(exe.join("components"));
        dirs.push(exe.join("data").join("components"));
    }
    dirs
}

/// Locates the OptiScaler component directory containing OptiScaler.dll, OptiScaler.ini, and OptiScaler subfolder.
pub fn find_optiscaler_payload() -> Option<PathBuf> {
    let mut candidates: Vec<(u64, PathBuf)> = Vec::new();
    let search_dirs = get_component_roots();

    for dir in &search_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().contains("optiscaler") && path.join("OptiScaler.dll").is_file() {
                        let score = score_optiscaler_dir(&name);
                        candidates.push((score, path));
                    }
                }
            }
        }
        if dir.join("OptiScaler.dll").is_file() {
            let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("OptiScaler");
            let score = score_optiscaler_dir(name);
            candidates.push((score, dir.clone()));
        }
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, p)| p)
}

/// Discovers the path to ReShade64.dll across standard locations.
pub fn find_reshade64_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("ReShade64.dll"));
        candidates.push(root.join("reshade-vulkan").join("ReShade64.dll"));
    }

    for c in candidates {
        if c.is_file() && crate::core::pe::inspect_pe(&c).is_some() {
            return Some(c);
        }
    }

    None
}

/// Discovers the path to ReShade32.dll across standard locations.
pub fn find_reshade32_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("ReShade32.dll"));
        candidates.push(root.join("reshade-vulkan").join("ReShade32.dll"));
    }

    for c in candidates {
        if c.is_file() && crate::core::pe::inspect_pe(&c).is_some() {
            return Some(c);
        }
    }

    None
}

/// Discovers the path to renodx-mfgunlock.addon64.
pub fn find_mfg_unlock_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("mfg-unlock-1.0").join("renodx-mfgunlock.addon64"));
        candidates.push(root.join("mfg-unlock-0.9").join("renodx-mfgunlock.addon64"));
        candidates.push(root.join("mfg-unlock-0.8").join("renodx-mfgunlock.addon64"));
        candidates.push(root.join("renodx-mfgunlock.addon64"));

        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("mfg-unlock-") {
                        candidates.push(p.join("renodx-mfgunlock.addon64"));
                    }
                }
            }
        }
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    None
}

pub fn find_mfg_addon_payload() -> Option<PathBuf> {
    find_mfg_unlock_payload()
}

/// Discovers the path to renodx-dlss5.addon64 or renodx-dlss.addon64.
pub fn find_renodx_dlss5_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("renodx-dlss5").join("renodx-dlss5.addon64"));
        candidates.push(root.join("renodx-dlss5.addon64"));
        candidates.push(root.join("renodx-dlss").join("renodx-dlss.addon64"));
        candidates.push(root.join("renodx-dlss.addon64"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    None
}

pub fn find_renodx_payload() -> Option<PathBuf> {
    find_renodx_dlss5_payload()
}

/// Discovers the path to dlss-mip-fix.addon64 or dlss5-d3d12-fix.addon64.
pub fn find_dlss5_d3d12_fix_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("dlss-mip-fix").join("dlss-mip-fix.addon64"));
        candidates.push(root.join("dlss-mip-fix.addon64"));
        candidates.push(root.join("dlss5-d3d12-fix").join("dlss5-d3d12-fix.addon64"));
        candidates.push(root.join("dlss5-d3d12-fix.addon64"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    None
}

/// Discovers nvngx_dlss.dll from Streamline component directory.
pub fn find_streamline_nvngx_dlss_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("streamline-2.14.1").join("streamline").join("nvngx_dlss.dll"));
        candidates.push(root.join("streamline").join("nvngx_dlss.dll"));
        candidates.push(root.join("nvngx_dlss.dll"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    None
}

pub fn find_dlss_payload() -> Option<PathBuf> {
    find_streamline_nvngx_dlss_payload()
}

/// Discovers nvngx_dlssnr.dll from Streamline component directory.
pub fn find_streamline_nvngx_dlssnr_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("streamline-2.14.1").join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(root.join("streamline").join("nvngx_dlssnr.dll"));
        candidates.push(root.join("nvngx_dlssnr.dll"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    None
}

pub fn find_dlssnr_payload() -> Option<PathBuf> {
    find_streamline_nvngx_dlssnr_payload()
}

/// Discovers standalone RTXMFG.dll.
pub fn find_standalone_rtxmfg_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("mfg-standalone").join("RTXMFG.dll"));
        candidates.push(root.join("mfg-standalone").join("RTX40MFG.dll"));
        candidates.push(root.join("RTXMFG-1.3.2").join("RTXMFG.dll"));
        candidates.push(root.join("RTXMFG.dll"));
        candidates.push(root.join("RTX40MFG.dll"));

        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string().to_lowercase();
                    if name.contains("mfg") || name.contains("rtx") {
                        candidates.push(p.join("RTXMFG.dll"));
                        candidates.push(p.join("RTX40MFG.dll"));
                    }
                }
            }
        }
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    None
}

pub fn find_standalone_mfg_payload() -> Option<PathBuf> {
    find_standalone_rtxmfg_payload()
}

/// Discovers the Streamline runtime interposer directory.
pub fn find_streamline_interposer_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("streamline-2.14.1").join("streamline"));
        candidates.push(root.join("streamline"));
    }

    for c in candidates {
        if c.is_dir() && c.join("sl.interposer.dll").is_file() && c.join("sl.common.dll").is_file() {
            return Some(c);
        }
    }

    None
}

pub fn find_streamline_payload() -> Option<PathBuf> {
    find_streamline_interposer_dir()
}

/// Discovers the DLSS-NR forwarder DLL (nvngx.dll_dlssnr.dll).
pub fn find_nvngx_dlssnr_forwarder_payload() -> Option<PathBuf> {
    for root in get_component_roots() {
        let p = root.join("nvngx.dll_dlssnr.dll");
        if p.is_file() {
            return Some(p);
        }
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let sub = entry.path();
                if sub.is_dir() {
                    let sub_p = sub.join("nvngx.dll_dlssnr.dll");
                    if sub_p.is_file() {
                        return Some(sub_p);
                    }
                }
            }
        }
    }

    None
}

pub fn find_nvngx_snippet_payload(opti_dir: &Path) -> Option<PathBuf> {
    let p = opti_dir.join("nvngx.dll_dlssnr.dll");
    if p.is_file() {
        Some(p)
    } else {
        find_nvngx_dlssnr_forwarder_payload()
    }
}

/// Discovers the path to dlss-overlay.addon64.
pub fn find_overlay_addon_payload() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    for root in get_component_roots() {
        candidates.push(root.join("dlss-overlay.addon64"));
        candidates.push(root.join("dlss5-lab-overlay.addon64"));
    }

    candidates.push(PathBuf::from(r"crates\addons\dlss-overlay-addon\assets\dlss-overlay.addon64"));
    candidates.push(PathBuf::from("crates/addons/dlss-overlay-addon/assets/dlss-overlay.addon64"));
    candidates.push(PathBuf::from(r"assets\dlss-overlay.addon64"));
    candidates.push(PathBuf::from("assets/dlss-overlay.addon64"));

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }

    None
}

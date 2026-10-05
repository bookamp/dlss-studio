//! Local on-disk cache discovery and validation for add-on runtimes.

use std::fs;
use std::path::{Path, PathBuf};

use super::AddonId;
use super::{DgVoodooComponents, FeederComponents};
use crate::core::state::get_appdata_dir;

pub fn get_components_root() -> PathBuf {
    let p = get_appdata_dir().join("components");
    let _ = fs::create_dir_all(&p);
    p
}

/// Discovers local dgVoodoo2 components on disk
pub fn find_local_dgvoodoo_components() -> Option<DgVoodooComponents> {
    let mut search_dirs = Vec::new();
    search_dirs.push(get_components_root());
    if let Ok(exe_p) = std::env::current_exe() {
        if let Some(exe_dir) = exe_p.parent() {
            search_dirs.push(exe_dir.join("components"));
            search_dirs.push(exe_dir.join("components").join("dgvoodoo"));
        }
    }

    for dir in search_dirs {
        let dg = if dir.join("dgvoodoo").is_dir() {
            dir.join("dgvoodoo")
        } else {
            dir
        };

        let d3d9_x86 = if dg.join("x86").join("D3D9.dll").is_file() {
            dg.join("x86").join("D3D9.dll")
        } else if dg.join("D3D9.dll").is_file() {
            dg.join("D3D9.dll")
        } else {
            continue;
        };

        let d3d9_x64 = if dg.join("x64").join("D3D9.dll").is_file() {
            dg.join("x64").join("D3D9.dll")
        } else if dg.join("D3D9_x64.dll").is_file() {
            dg.join("D3D9_x64.dll")
        } else {
            d3d9_x86.clone()
        };

        let d3d8_x86 = if dg.join("x86").join("D3D8.dll").is_file() {
            Some(dg.join("x86").join("D3D8.dll"))
        } else if dg.join("D3D8.dll").is_file() {
            Some(dg.join("D3D8.dll"))
        } else {
            None
        };

        let conf = if dg.join("dgVoodoo.conf").is_file() {
            dg.join("dgVoodoo.conf")
        } else {
            continue;
        };

        return Some(DgVoodooComponents {
            d3d9_x86,
            d3d9_x64,
            d3d8_x86,
            conf,
        });
    }
    None
}

/// Discovers local Feeder components (addon64, addon32, host64, shaders, vk_layer) on disk
pub fn find_local_feeder_components() -> Option<FeederComponents> {
    let mut search_dirs = Vec::new();
    search_dirs.push(get_components_root());
    if let Ok(exe_p) = std::env::current_exe() {
        if let Some(exe_dir) = exe_p.parent() {
            search_dirs.push(exe_dir.join("components"));
            search_dirs.push(exe_dir.join("components").join("feeder"));
        }
    }

    for dir in search_dirs {
        if let Some(comp) = find_feeder_components_in_dir(&dir) {
            return Some(comp);
        }
    }
    None
}

/// Searches for Feeder components inside a specific root directory
pub fn find_feeder_components_in_dir(dir: &Path) -> Option<FeederComponents> {
    // Collect potential feeder candidate directories
    let mut candidate_subdirs = Vec::new();
    candidate_subdirs.push(dir.to_path_buf());
    candidate_subdirs.push(dir.join("dlss5-feeder"));

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("DLSS5-Feeder-") || name.starts_with("dlss5-feeder") {
                    candidate_subdirs.push(p);
                }
            }
        }
    }

    // Sort descending so newest versioned folder is checked first
    candidate_subdirs.sort_by(|a, b| b.cmp(a));

    for c_dir in candidate_subdirs {
        let addon64 = if c_dir.join("dlss5-feed.addon64").is_file() {
            Some(c_dir.join("dlss5-feed.addon64"))
        } else if dir.join("dlss5-feed.addon64").is_file() {
            Some(dir.join("dlss5-feed.addon64"))
        } else {
            None
        };

        if let Some(addon64) = addon64 {
            let is_valid_feeder_shader_dir = |p: &std::path::Path| -> bool {
                p.join("Shaders").join("DLSS5_Feed.fx").is_file()
                    && p.join("Textures").join("vort_BlueNoise.png").is_file()
            };

            let shader_dir = if is_valid_feeder_shader_dir(&dir.join("feeder-shaders")) {
                dir.join("feeder-shaders")
            } else if is_valid_feeder_shader_dir(&dir.join("reshade-shaders")) {
                dir.join("reshade-shaders")
            } else if is_valid_feeder_shader_dir(&c_dir.join("reshade-shaders")) {
                c_dir.join("reshade-shaders")
            } else {
                continue;
            };

            let vk_layer_dir = if c_dir.join("layer-x64").join("VkLayer_feed_vk.dll").is_file() {
                Some(c_dir.join("layer-x64"))
            } else if dir.join("layer-x64").join("VkLayer_feed_vk.dll").is_file() {
                Some(dir.join("layer-x64"))
            } else {
                None
            };

            let addon32 = if c_dir.join("dlss5-feed.addon32").is_file() {
                Some(c_dir.join("dlss5-feed.addon32"))
            } else if dir.join("dlss5-feed.addon32").is_file() {
                Some(dir.join("dlss5-feed.addon32"))
            } else {
                None
            };

            let host64 = if c_dir.join("dlss5-feed-host64.exe").is_file() {
                Some(c_dir.join("dlss5-feed-host64.exe"))
            } else if c_dir.join("host64").join("dlss5-feed-host64.exe").is_file() {
                Some(c_dir.join("host64").join("dlss5-feed-host64.exe"))
            } else if dir.join("dlss5-feed-host64.exe").is_file() {
                Some(dir.join("dlss5-feed-host64.exe"))
            } else {
                None
            };

            return Some(FeederComponents {
                addon64,
                addon32,
                host64,
                shader_dir,
                vk_layer_dir,
            });
        }
    }
    None
}

/// Checks whether dgVoodoo2 is cached on disk
pub fn is_dgvoodoo_cached() -> bool {
    find_local_dgvoodoo_components().is_some()
}

/// Checks whether the RenoDX 4x MFG Unlock addon is cached on disk
pub fn is_mfg_addon_cached() -> bool {
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        if root.join("mfg-unlock-1.0").join("renodx-mfgunlock.addon64").is_file()
            || root.join("mfg-unlock-0.9").join("renodx-mfgunlock.addon64").is_file()
            || root.join("mfg-unlock-0.8").join("renodx-mfgunlock.addon64").is_file()
            || root.join("renodx-mfgunlock.addon64").is_file()
        {
            return true;
        }
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("mfg-unlock-") && p.join("renodx-mfgunlock.addon64").is_file() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Checks whether the DLSS 5 Feeder and shaders are cached on disk
pub fn is_feeder_cached() -> bool {
    find_local_feeder_components().is_some()
}

/// Checks whether the RenoDX Native DLSS 5 engine is cached on disk
pub fn is_renodx_engine_cached() -> bool {
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
            if parent.join("renodx-dlss5.addon64").is_file()
                || parent.join("renodx-dlss.addon64").is_file()
            {
                return true;
            }
        }
    }
    for root in roots {
        if root.join("renodx-dlss5").join("renodx-dlss5.addon64").is_file()
            || root.join("renodx-dlss5.addon64").is_file()
            || root.join("renodx-dlss").join("renodx-dlss.addon64").is_file()
            || root.join("renodx-dlss.addon64").is_file()
        {
            return true;
        }
    }
    false
}

/// Checks whether the D3D12 Mip Fix companion is cached on disk
pub fn is_dlss5_d3d12_fix_cached() -> bool {
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
            if parent.join("dlss-mip-fix.addon64").is_file() {
                return true;
            }
        }
    }
    for root in roots {
        if root.join("dlss-mip-fix").join("dlss-mip-fix.addon64").is_file()
            || root.join("dlss-mip-fix.addon64").is_file()
            || root.join("dlss5-d3d12-fix").join("dlss5-d3d12-fix.addon64").is_file()
            || root.join("dlss5-d3d12-fix.addon64").is_file()
        {
            return true;
        }
    }
    false
}

/// Checks whether Standalone RTX40MFG-Unlock (Dashdogy) is cached on disk
pub fn is_rtxmfg_cached() -> bool {
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        if root.join("mfg-standalone").join("RTXMFG.dll").is_file()
            || root.join("RTXMFG.dll").is_file()
        {
            return true;
        }
    }
    false
}

/// Checks whether the OptiScaler runtime is cached on disk (supporting any version dynamically)
pub fn is_optiscaler_cached() -> bool {
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        if root.join("OptiScaler.dll").is_file() {
            return true;
        }
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("OptiScaler.dll").is_file() {
                    return true;
                }
            }
        }
    }
    false
}

/// Checks whether Streamline Runtime is cached on disk
pub fn is_streamline_cached() -> bool {
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        let streamline_dir = root.join("streamline-2.14.1").join("streamline");
        if streamline_dir.join("sl.interposer.dll").is_file() && streamline_dir.join("sl.common.dll").is_file() {
            return true;
        }
        if root.join("sl.interposer.dll").is_file() && root.join("sl.common.dll").is_file() {
            return true;
        }
    }
    false
}

/// Checks whether ReShade 6.8.0 runtime (both 64-bit and 32-bit) is cached on disk and verified as genuine PE binaries
pub fn is_reshade_cached() -> bool {
    let is_valid_reshade = |p: &PathBuf| -> bool {
        p.is_file() && crate::core::pe::inspect_pe(p).is_some()
    };
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        let has_64 = is_valid_reshade(&root.join("ReShade64.dll")) || is_valid_reshade(&root.join("reshade-vulkan").join("ReShade64.dll"));
        let has_32 = is_valid_reshade(&root.join("ReShade32.dll")) || is_valid_reshade(&root.join("reshade-vulkan").join("ReShade32.dll"));
        if has_64 && has_32 {
            return true;
        }
    }
    false
}

/// Checks whether a specific addon is cached
pub fn is_addon_cached(id: AddonId) -> bool {
    match id {
        AddonId::OptiScaler => is_optiscaler_cached(),
        AddonId::ReShade => is_reshade_cached(),
        AddonId::Dlss5Feeder => is_feeder_cached(),
        AddonId::RenoDxMfgUnlock => is_mfg_addon_cached(),
        AddonId::RenoDxDlss5 => is_renodx_engine_cached(),
        AddonId::DlssMipFix => is_dlss5_d3d12_fix_cached(),
        AddonId::Rtx40Mfg => is_rtxmfg_cached(),
        AddonId::Streamline => is_streamline_cached(),
        AddonId::DgVoodoo => is_dgvoodoo_cached(),
    }
}

/// Returns true if all mandatory components are cached locally
pub fn are_all_mandatory_components_cached() -> bool {
    is_mfg_addon_cached()
        && is_feeder_cached()
        && is_renodx_engine_cached()
        && is_dlss5_d3d12_fix_cached()
        && is_rtxmfg_cached()
        && is_optiscaler_cached()
        && is_streamline_cached()
        && is_reshade_cached()
}

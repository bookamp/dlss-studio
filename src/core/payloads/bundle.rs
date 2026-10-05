//! Runtime component bundle representations.

use std::path::PathBuf;

use super::discovery::*;

// Release metadata for bundled and managed payloads
pub const OPTISCALER_RELEASE_VERSION: &str = "0.7.6-dlssnr";
pub const MFG_UNLOCK_RELEASE_VERSION: &str = "0.9";
pub const MFG_UNLOCK_RELEASE_URL: &str = "https://github.com/mavismmg/MFGAdaUnlock-RenoDx/releases/download/0.9/renodx-mfgunlock.addon64";
pub const MFG_UNLOCK_RELEASE_SHA256: &str = "64184bb370f223c3cabb359010a9a64e114cdae6b62d8b014a731a602af0a0da";

/// Fully decoupled, typed bundle of required runtime component binaries.
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
    pub feeder_components: Option<crate::core::addons::FeederComponents>,
    pub dgvoodoo: Option<crate::core::addons::DgVoodooComponents>,
}

impl PayloadBundle {
    pub fn from_system() -> Result<Self, String> {
        let opti_dir = find_optiscaler_payload();
        let (optiscaler_dll, optiscaler_ini, optiscaler_dir) = if let Some(ref dir) = opti_dir {
            (
                dir.join("OptiScaler.dll"),
                dir.join("OptiScaler.ini"),
                if dir.join("OptiScaler").is_dir() {
                    Some(dir.join("OptiScaler"))
                } else {
                    None
                },
            )
        } else {
            (PathBuf::new(), PathBuf::new(), None)
        };
        let nvngx_snippet_dll = opti_dir.as_deref().and_then(find_nvngx_snippet_payload);
        let nvngx_dlss_dll = find_dlss_payload();
        let nvngx_dlssnr_dll = find_dlssnr_payload();
        let rtxmfg_dll = find_standalone_mfg_payload();
        let reshade64_dll = find_reshade64_payload();
        let reshade32_dll = find_reshade32_payload();
        let renodx_dlss5_addon = find_renodx_payload();
        let dlss5_d3d12_fix_addon = find_dlss5_d3d12_fix_payload();
        let renodx_mfgunlock_addon = find_mfg_addon_payload();
        let streamline_dir = find_streamline_payload();
        let feeder_components = crate::core::addons::find_local_feeder_components();
        let dgvoodoo = crate::core::addons::find_local_dgvoodoo_components();

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

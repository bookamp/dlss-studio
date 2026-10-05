pub mod cache;
pub mod catalog;
pub mod installer;
pub mod version;

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AddonId {
    RenoDxDlss5,
    DlssMipFix,
    RenoDxMfgUnlock,
    Dlss5Feeder,
    Rtx40Mfg,
    OptiScaler,
    ReShade,
    Streamline,
    DgVoodoo,
}

pub const ALL_REFERENCED_ADDONS: &[AddonId] = &[
    AddonId::RenoDxDlss5,
    AddonId::DlssMipFix,
    AddonId::RenoDxMfgUnlock,
    AddonId::Dlss5Feeder,
    AddonId::Rtx40Mfg,
    AddonId::OptiScaler,
    AddonId::ReShade,
    AddonId::Streamline,
    AddonId::DgVoodoo,
];

impl AddonId {
    pub fn metadata(&self) -> catalog::AddonMetadata {
        catalog::get_metadata(*self)
    }

    pub fn is_cached(&self) -> bool {
        cache::is_addon_cached(*self)
    }

    pub fn name(&self) -> &'static str {
        self.metadata().name
    }

    pub fn description_key(&self) -> &'static str {
        self.metadata().description_key
    }

    pub fn url(&self) -> &'static str {
        self.metadata().url
    }
}

// Re-export common component types and functions for seamless integration
pub use cache::{
    are_all_mandatory_components_cached, find_local_dgvoodoo_components,
    find_local_feeder_components, get_components_root, is_addon_cached,
    is_dgvoodoo_cached, is_dlss5_d3d12_fix_cached, is_feeder_cached,
    is_mfg_addon_cached, is_optiscaler_cached, is_renodx_engine_cached,
    is_reshade_cached, is_rtxmfg_cached, is_streamline_cached,
};

pub use catalog::{
    resolve_github_latest_asset, resolve_latest_dashdogy_mfg,
    resolve_latest_optiscaler, resolve_latest_optiscaler_mfg, AddonMetadata,
    DGVOODOO_SHA256, DGVOODOO_URL, DLSS5_FEEDER_RELEASE_URL, DLSS_MIP_FIX_URL,
    DLSS_NR_URL, DRAWTEXT_FXH_SHA256, DRAWTEXT_FXH_URL, FEEDER_ARCHIVE_SHA256,
    FEEDER_ARCHIVE_URL, FONTATLAS_PNG_SHA256, FONTATLAS_PNG_URL, MFG_UNLOCK_SHA256,
    MFG_UNLOCK_URL, OPTISCALER_FALLBACK_URL, OPTISCALER_MFG_FALLBACK_URL,
    OPTISCALER_SHA256, OPTISCALER_REPO, OPTISCALER_REPO_URL, OPTISCALER_URL, RENODX_DLSS5_URL, RESHADE_FXH_SHA256,
    RESHADE_FXH_URL, RESHADE_SETUP_SHA256, RESHADE_SETUP_URL,
    RESHADE_SHADERS_SLIM_SHA256, RESHADE_SHADERS_SLIM_URL, RESHADE_UI_FXH_SHA256,
    RESHADE_UI_FXH_URL, RTX40MFG_FALLBACK_URL, RTX40MFG_URL, STREAMLINE_ZIP_URL,
    VORT_ARCHIVE_SHA256, VORT_ARCHIVE_URL,
};

pub use installer::{
    ensure_all_mandatory_components_with_progress, ensure_dgvoodoo_components,
    ensure_dlss5_d3d12_fix_addon, ensure_feeder_components, ensure_mfg_v09_addon,
    ensure_mandatory_components_cached_with_progress, extract_reshade_from_setup,
    install_or_update_addon, update_all_addons, update_single_addon,
};

pub use version::{
    check_addon_update, check_all_addon_updates, check_all_addon_updates_force,
    get_local_addon_version, is_newer_version, load_version_manifest,
    record_addon_version, save_version_manifest, AddonVersionInfo,
    ComponentsVersionManifest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgVoodooComponents {
    pub d3d9_x86: PathBuf,
    pub d3d9_x64: PathBuf,
    pub d3d8_x86: Option<PathBuf>,
    pub conf: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeederComponents {
    pub addon64: PathBuf,
    pub addon32: Option<PathBuf>,
    pub host64: Option<PathBuf>,
    pub shader_dir: PathBuf,
    pub vk_layer_dir: Option<PathBuf>,
}

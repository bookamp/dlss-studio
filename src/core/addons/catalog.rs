use super::AddonId;

pub const DLSS5_FEEDER_RELEASE_URL: &str = FEEDER_ARCHIVE_URL;
pub const FEEDER_ARCHIVE_URL: &str = "https://github.com/jlrouzies-fr/DLSS5-Feeder/releases/download/v1.16.0-beta.3/DLSS5-Feeder-1.16.0-beta.3.zip";
pub const FEEDER_ARCHIVE_SHA256: &str = "f2560fe2ef1521451dbe5c48b1bfb91eb0023a10ae57d23d8eb5363ec946ee99";

pub const VORT_ARCHIVE_URL: &str = "https://codeload.github.com/vortigern11/vort_Shaders/zip/b410b9f0c0fbb83c8cb42164aaf1655fab386f4a";
pub const VORT_ARCHIVE_SHA256: &str = "231ba34a75556f9943e359559a89b0d0cc2caa322d9dcdee5630061bf9fe13b6";

pub const RESHADE_SHADERS_SLIM_URL: &str = "https://github.com/crosire/reshade-shaders/archive/6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc.zip";
pub const RESHADE_SHADERS_SLIM_SHA256: &str = "63d76e33ea2ce683d789e0238515570020bc8fbc5f6b215865e94b4e72ce9f1d";

pub const RESHADE_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/ReShade.fxh";
pub const RESHADE_FXH_SHA256: &str = "c2538cb887a71fec739ec308ff28189ae4e2c8be373b5eb4b988f01c385ad2f7";

pub const RESHADE_UI_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/ReShadeUI.fxh";
pub const RESHADE_UI_FXH_SHA256: &str = "8db2e5d95e263d91cf0fc5b69c735d64eb0766324e9334861cb7e5d8ff66ec48";

pub const DRAWTEXT_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/DrawText.fxh";
pub const DRAWTEXT_FXH_SHA256: &str = "b79cc4dfb3e98bcf4c06193d00ea7631d74f467f73a4deeeee13e71336d3e680";

pub const FONTATLAS_PNG_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Textures/FontAtlas.png";
pub const FONTATLAS_PNG_SHA256: &str = "11a711a8167d1c1606892e6fa6f661a477e749d6cbdb1ff700ac381842066ec3";

pub const MFG_UNLOCK_URL: &str = "https://github.com/mavismmg/MFGAdaUnlock-RenoDx/releases/download/1.0/renodx-mfgunlock.addon64";
pub const MFG_UNLOCK_SHA256: &str = "f9f10c685e3e89077f751df2394a1629615a56b58d111dff26b39894e772d50e";

pub const RTX40MFG_URL: &str = RTX40MFG_FALLBACK_URL;
pub const RTX40MFG_FALLBACK_URL: &str = "https://github.com/dashdogy/RTX40MFG-Unlock/releases/download/v1.3.3-hotfix.2/RTXMFG-v1.3.3-hotfix.2.zip";
pub const OPTISCALER_REPO: &str = "wilsjo2/OptiScaler-DLSSNR-PreSR-Multipass";
pub const OPTISCALER_REPO_URL: &str = "https://github.com/wilsjo2/OptiScaler-DLSSNR-PreSR-Multipass";
pub const OPTISCALER_URL: &str = "https://github.com/wilsjo2/OptiScaler-DLSSNR-PreSR-Multipass/releases/download/v0.8.91/OptiScaler-NR-v0.8.91-rtx40-mfg.zip";
pub const OPTISCALER_SHA256: &str = "93e6762ed3ab9f48a2f78b088346786383315a9916c32440f0fe6457422fad79";
pub const OPTISCALER_FALLBACK_URL: &str = OPTISCALER_URL;
pub const OPTISCALER_MFG_FALLBACK_URL: &str = OPTISCALER_URL;

pub const DLSS_MIP_FIX_URL: &str = "https://github.com/bookamp/dlss-studio/releases/latest/download/dlss-mip-fix.addon64";
pub const DLSS_NR_URL: &str = "https://github.com/bookamp/dlss-studio/releases/latest/download/dlss-nr.addon64";

pub const RENODX_DLSS5_URL: &str = "https://github.com/yumlevi/renodx-dlss-installer/releases/download/latest/renodx-dlss5.addon64";
pub const STREAMLINE_ZIP_URL: &str = "https://github.com/yumlevi/renodx-dlss-installer/releases/download/latest/streamline.zip";
pub const RESHADE_SETUP_URL: &str = "https://reshade.me/downloads/ReShade_Setup_6.8.0_Addon.exe";
pub const RESHADE_SETUP_SHA256: &str = "afe4c8f13048306307983b8b3d41d5bf00a86820440b0e57dea10950e1176445";

pub const DGVOODOO_URL: &str = "https://github.com/dege-diosg/dgVoodoo2/releases/download/v2.87.5/dgVoodoo2_87_5.zip";
pub const DGVOODOO_SHA256: &str = "5ffde6927f7355ca3fdd5d785b581256a8e6539fa13e395a891ade6ba1040850";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonMetadata {
    pub id: AddonId,
    pub name: &'static str,
    pub description_key: &'static str,
    pub url: &'static str,
    pub expected_sha256: &'static str,
    pub mandatory: bool,
    pub target_filename: &'static str,
}

pub fn get_metadata(id: AddonId) -> AddonMetadata {
    match id {
        AddonId::RenoDxDlss5 => AddonMetadata {
            id,
            name: "RenoDX v4.7 (Integrated DLSS 5 Engine)",
            description_key: "addon_renodx_desc",
            url: RENODX_DLSS5_URL,
            expected_sha256: "",
            mandatory: true,
            target_filename: "renodx-dlss5.addon64",
        },
        AddonId::DlssMipFix => AddonMetadata {
            id,
            name: "DLSS Studio D3D12 Mip Companion",
            description_key: "addon_dlss5_d3d12_fix_desc",
            url: DLSS_MIP_FIX_URL,
            expected_sha256: "",
            mandatory: true,
            target_filename: "dlss-mip-fix.addon64",
        },
        AddonId::RenoDxMfgUnlock => AddonMetadata {
            id,
            name: "RenoDX 4x MFG Unlock v1.0",
            description_key: "addon_mfg_desc",
            url: MFG_UNLOCK_URL,
            expected_sha256: MFG_UNLOCK_SHA256,
            mandatory: true,
            target_filename: "renodx-mfgunlock.addon64",
        },
        AddonId::Dlss5Feeder => AddonMetadata {
            id,
            name: "DLSS 5 Feeder (Neural Pipeline Interceptor)",
            description_key: "addon_feeder_desc",
            url: FEEDER_ARCHIVE_URL,
            expected_sha256: FEEDER_ARCHIVE_SHA256,
            mandatory: true,
            target_filename: "dlss5-feed.addon64",
        },
        AddonId::Rtx40Mfg => AddonMetadata {
            id,
            name: "Universal RTX40MFG-Unlock (Dashdogy)",
            description_key: "tag_rtx40",
            url: RTX40MFG_FALLBACK_URL,
            expected_sha256: "",
            mandatory: true,
            target_filename: "RTXMFG.dll",
        },
        AddonId::OptiScaler => AddonMetadata {
            id,
            name: "OptiScaler DLSS-NR Pre-SR Multi-pass",
            description_key: "addon_optiscaler_desc",
            url: OPTISCALER_URL,
            expected_sha256: OPTISCALER_SHA256,
            mandatory: true,
            target_filename: "OptiScaler.dll",
        },
        AddonId::ReShade => AddonMetadata {
            id,
            name: "ReShade 6.8.0 (Add-on Support)",
            description_key: "addon_reshade_desc",
            url: RESHADE_SETUP_URL,
            expected_sha256: RESHADE_SETUP_SHA256,
            mandatory: true,
            target_filename: "ReShade64.dll",
        },
        AddonId::Streamline => AddonMetadata {
            id,
            name: "Streamline Runtime v2.14.1",
            description_key: "addon_streamline_desc",
            url: STREAMLINE_ZIP_URL,
            expected_sha256: "",
            mandatory: true,
            target_filename: "sl.interposer.dll",
        },
        AddonId::DgVoodoo => AddonMetadata {
            id,
            name: "dgVoodoo2 v2.87.5 (Legacy D3D -> D3D11)",
            description_key: "addon_dgvoodoo_desc",
            url: DGVOODOO_URL,
            expected_sha256: DGVOODOO_SHA256,
            mandatory: true,
            target_filename: "D3D9.dll",
        },
    }
}

/// Queries GitHub API for the latest release asset matching a pattern.
/// Returns (download_url, tag_name) or Err.
pub async fn resolve_github_latest_asset<F>(repo: &str, matcher: F) -> Result<(String, String), String>
where
    F: Fn(&str) -> bool,
{
    let client = reqwest::Client::builder()
        .user_agent(concat!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) DLSS-Studio/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("https://api.github.com/repos/{}/releases", repo);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API {} returned HTTP {}", url, resp.status()));
    }
    let releases: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;
    for rel in releases {
        let tag = rel.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if let Some(assets) = rel.get("assets").and_then(|v| v.as_array()) {
            for asset in assets {
                let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if matcher(name) {
                    if let Some(dl_url) = asset.get("browser_download_url").and_then(|v| v.as_str()) {
                        return Ok((dl_url.to_string(), tag));
                    }
                }
            }
        }
    }
    Err("No matching asset found in releases".to_string())
}

/// Resolves the latest download URL and tag for Dashdogy's RTX40MFG-Unlock dynamically
pub async fn resolve_latest_dashdogy_mfg() -> (String, String) {
    if let Ok((url, tag)) = resolve_github_latest_asset("dashdogy/RTX40MFG-Unlock", |name| {
        name.starts_with("RTXMFG-") && name.ends_with(".zip")
    }).await {
        return (url, tag);
    }
    (
        RTX40MFG_FALLBACK_URL.to_string(),
        "v1.3.3-hotfix.2".to_string(),
    )
}

/// Resolves the latest download URL and tag for OptiScaler dynamically
pub async fn resolve_latest_optiscaler() -> (String, String) {
    if let Ok((url, tag)) = resolve_github_latest_asset("wilsjo2/OptiScaler-DLSSNR-PreSR-Multipass", |name| {
        (name.starts_with("OptiScaler-NR-") || name.starts_with("OptiScaler-DLSSNR-") || name.starts_with("OptiScaler-")) && name.ends_with(".zip")
    }).await {
        return (url, tag);
    }
    (
        OPTISCALER_FALLBACK_URL.to_string(),
        "v0.8.91".to_string(),
    )
}

/// Helper alias
pub async fn resolve_latest_optiscaler_mfg() -> (String, String) {
    resolve_latest_optiscaler().await
}

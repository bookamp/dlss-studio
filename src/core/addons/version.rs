use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

use super::cache::{get_components_root, is_addon_cached};
use super::catalog::resolve_github_latest_asset;
use super::{AddonId, ALL_REFERENCED_ADDONS};
use crate::core::pe::inspect_pe;

const VERSION_CACHE_TTL: Duration = Duration::from_secs(15 * 60);

static ADDON_UPDATE_CACHE: OnceLock<Mutex<Option<(Instant, Vec<AddonVersionInfo>)>>> = OnceLock::new();

fn get_cache_mutex() -> &'static Mutex<Option<(Instant, Vec<AddonVersionInfo>)>> {
    ADDON_UPDATE_CACHE.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_addon_cache() {
    if let Ok(mut lock) = get_cache_mutex().lock() {
        *lock = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddonVersionInfo {
    pub id: AddonId,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentsVersionManifest {
    #[serde(default)]
    pub versions: HashMap<String, String>,
    #[serde(default)]
    pub last_checked: Option<u64>,
}

fn manifest_path() -> PathBuf {
    get_components_root().join("components_version.json")
}

pub fn load_version_manifest() -> ComponentsVersionManifest {
    let p = manifest_path();
    if p.is_file() {
        if let Ok(data) = fs::read_to_string(&p) {
            if let Ok(manifest) = serde_json::from_str::<ComponentsVersionManifest>(&data) {
                return manifest;
            }
        }
    }
    ComponentsVersionManifest::default()
}

pub fn save_version_manifest(manifest: &ComponentsVersionManifest) -> Result<(), String> {
    let p = manifest_path();
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    fs::write(&p, data).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn record_addon_version(id: AddonId, version: &str) -> Result<(), String> {
    let mut manifest = load_version_manifest();
    manifest.versions.insert(id_key(id), version.to_string());
    save_version_manifest(&manifest)
}

fn id_key(id: AddonId) -> String {
    match id {
        AddonId::RenoDxDlss5 => "renodx_dlss5".to_string(),
        AddonId::DlssMipFix => "dlss_mip_fix".to_string(),
        AddonId::RenoDxMfgUnlock => "renodx_mfg_unlock".to_string(),
        AddonId::Dlss5Feeder => "dlss5_feeder".to_string(),
        AddonId::Rtx40Mfg => "rtx40_mfg".to_string(),
        AddonId::OptiScaler => "optiscaler".to_string(),
        AddonId::ReShade => "reshade".to_string(),
        AddonId::Streamline => "streamline".to_string(),
        AddonId::DgVoodoo => "dgvoodoo".to_string(),
    }
}

pub fn get_local_addon_version(id: AddonId) -> Option<String> {
    if !is_addon_cached(id) {
        return None;
    }

    let manifest = load_version_manifest();
    if let Some(ver) = manifest.versions.get(&id_key(id)) {
        if !ver.is_empty() {
            return Some(ver.clone());
        }
    }

    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        match id {
            AddonId::RenoDxDlss5 => {
                let p = root.join("renodx-dlss5.addon64");
                let p_sub = root.join("renodx-dlss5").join("renodx-dlss5.addon64");
                let target = if p.is_file() { p } else { p_sub };
                if let Some(pe) = inspect_pe(&target) {
                    if let Some(v) = pe.version {
                        return Some(format!("v{}", v.trim_start_matches('v')));
                    }
                }
            }
            AddonId::DlssMipFix => {
                let p = root.join("dlss-mip-fix.addon64");
                let p_sub = root.join("dlss-mip-fix").join("dlss-mip-fix.addon64");
                let target = if p.is_file() { p } else { p_sub };
                if let Some(pe) = inspect_pe(&target) {
                    if let Some(v) = pe.version {
                        return Some(format!("v{}", v.trim_start_matches('v')));
                    }
                }
            }
            AddonId::RenoDxMfgUnlock => {
                let mut versions = Vec::new();
                if let Ok(entries) = fs::read_dir(&root) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("mfg-unlock-") {
                            let ver = name.trim_start_matches("mfg-unlock-");
                            versions.push(if ver.starts_with('v') { ver.to_string() } else { format!("v{}", ver) });
                        }
                    }
                }
                if let Some(highest) = versions.into_iter().max_by(|a, b| {
                    if is_newer_version(a, b) {
                        std::cmp::Ordering::Less
                    } else if is_newer_version(b, a) {
                        std::cmp::Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                }) {
                    return Some(highest);
                }
            }
            AddonId::Dlss5Feeder => {
                let mut versions = Vec::new();
                if let Ok(entries) = fs::read_dir(&root) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("DLSS5-Feeder-") {
                            let ver = name.trim_start_matches("DLSS5-Feeder-");
                            versions.push(if ver.starts_with('v') { ver.to_string() } else { format!("v{}", ver) });
                        }
                    }
                }
                if let Some(highest) = versions.into_iter().max_by(|a, b| {
                    if is_newer_version(a, b) {
                        std::cmp::Ordering::Less
                    } else if is_newer_version(b, a) {
                        std::cmp::Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                }) {
                    return Some(highest);
                }
            }
            AddonId::Rtx40Mfg => {
                let mut versions = Vec::new();
                if let Ok(entries) = fs::read_dir(&root) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("RTXMFG-") {
                            let ver = name.trim_start_matches("RTXMFG-").trim_end_matches(".zip");
                            versions.push(if ver.starts_with('v') { ver.to_string() } else { format!("v{}", ver) });
                        }
                    }
                }
                if let Some(highest) = versions.into_iter().max_by(|a, b| {
                    if is_newer_version(a, b) {
                        std::cmp::Ordering::Less
                    } else if is_newer_version(b, a) {
                        std::cmp::Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                }) {
                    return Some(highest);
                }
            }
            AddonId::OptiScaler => {
                // 1. Inspect PE headers
                let opti_dll = root.join("OptiScaler.dll");
                let opti_sub_dll = root.join("optiscaler").join("OptiScaler.dll");
                let target = if opti_dll.is_file() { opti_dll } else { opti_sub_dll };
                if let Some(pe) = inspect_pe(&target) {
                    if let Some(v) = pe.version {
                        return Some(format!("v{}", v.trim_start_matches('v')));
                    }
                }
                // 2. Inspect directory entries matching OptiScaler-
                let mut versions = Vec::new();
                if let Ok(entries) = fs::read_dir(&root) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("OptiScaler-") {
                            let clean = name.trim_end_matches(".zip").trim_end_matches("-rtx40-mfg").trim_end_matches("-dlssnr");
                            let raw_ver = clean.trim_start_matches("OptiScaler-").trim_start_matches("NR-").trim_start_matches("dlssnr-");
                            let ver_str = if raw_ver.starts_with('v') { raw_ver.to_string() } else { format!("v{}", raw_ver) };
                            if !ver_str.is_empty() && ver_str != "v" {
                                versions.push(ver_str);
                            }
                        }
                    }
                }
                if let Some(highest) = versions.into_iter().max_by(|a, b| {
                    if is_newer_version(a, b) {
                        std::cmp::Ordering::Less
                    } else if is_newer_version(b, a) {
                        std::cmp::Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                }) {
                    return Some(highest);
                }
            }
            _ => {}
        }
    }

    match id {
        AddonId::RenoDxDlss5 => Some("v4.7".to_string()),
        AddonId::DlssMipFix => Some("v1.0.0".to_string()),
        AddonId::RenoDxMfgUnlock => Some("v1.0".to_string()),
        AddonId::Dlss5Feeder => Some("v1.16.0-beta.3".to_string()),
        AddonId::Rtx40Mfg => Some("v1.3.3-hotfix.2".to_string()),
        AddonId::OptiScaler => Some("v0.8.4".to_string()),
        AddonId::ReShade => Some("v6.8.0".to_string()),
        AddonId::Streamline => Some("v2.14.1".to_string()),
        AddonId::DgVoodoo => Some("v2.87.5".to_string()),
    }
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let c_clean = current.trim_start_matches('v').trim();
    let l_clean = latest.trim_start_matches('v').trim();
    if c_clean == l_clean {
        return false;
    }

    let parse_parts = |s: &str| -> (Vec<u64>, String) {
        let (num_part, suffix) = match s.find('-') {
            Some(idx) => (&s[..idx], s[idx..].to_string()),
            None => (s, String::new()),
        };
        let nums = num_part
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>();
        (nums, suffix)
    };

    let (c_nums, c_suf) = parse_parts(c_clean);
    let (l_nums, l_suf) = parse_parts(l_clean);

    let max_len = c_nums.len().max(l_nums.len());
    for i in 0..max_len {
        let c_val = *c_nums.get(i).unwrap_or(&0);
        let l_val = *l_nums.get(i).unwrap_or(&0);
        if l_val > c_val {
            return true;
        } else if l_val < c_val {
            return false;
        }
    }

    if !c_suf.is_empty() && l_suf.is_empty() {
        return true;
    }
    if c_suf.is_empty() && !l_suf.is_empty() {
        return false;
    }

    l_suf > c_suf
}

pub async fn check_addon_update(id: AddonId) -> AddonVersionInfo {
    let current_ver = get_local_addon_version(id);
    let (latest_tag, dl_url) = match id {
        AddonId::RenoDxDlss5 => {
            if let Ok((url, tag)) = resolve_github_latest_asset("yumlevi/renodx-dlss-installer", |name| {
                name.ends_with(".addon64") && name.contains("renodx")
            }).await {
                (Some(tag), Some(url))
            } else {
                (None, None)
            }
        }
        AddonId::DlssMipFix => {
            if let Ok((url, tag)) = resolve_github_latest_asset("bookamp/dlss-studio", |name| {
                name.contains("dlss-mip-fix")
            }).await {
                (Some(tag), Some(url))
            } else {
                (None, None)
            }
        }
        AddonId::RenoDxMfgUnlock => {
            if let Ok((url, tag)) = resolve_github_latest_asset("mavismmg/MFGAdaUnlock-RenoDx", |name| {
                name.ends_with(".addon64")
            }).await {
                (Some(tag), Some(url))
            } else {
                (None, None)
            }
        }
        AddonId::Dlss5Feeder => {
            if let Ok((url, tag)) = resolve_github_latest_asset("jlrouzies-fr/DLSS5-Feeder", |name| {
                name.starts_with("DLSS5-Feeder-") && name.ends_with(".zip")
            }).await {
                (Some(tag), Some(url))
            } else {
                (None, None)
            }
        }
        AddonId::Rtx40Mfg => {
            let (url, tag) = super::catalog::resolve_latest_dashdogy_mfg().await;
            (Some(tag), Some(url))
        }
        AddonId::OptiScaler => {
            let (url, tag) = super::catalog::resolve_latest_optiscaler_mfg().await;
            (Some(tag), Some(url))
        }
        AddonId::ReShade => (Some("v6.8.0".to_string()), Some(super::catalog::RESHADE_SETUP_URL.to_string())),
        AddonId::Streamline => (Some("v2.14.1".to_string()), Some(super::catalog::STREAMLINE_ZIP_URL.to_string())),
        AddonId::DgVoodoo => {
            if let Ok((url, tag)) = resolve_github_latest_asset("dege-diosg/dgVoodoo2", |name| {
                name.starts_with("dgVoodoo2_") && name.ends_with(".zip")
            }).await {
                (Some(tag), Some(url))
            } else {
                (Some("v2.87.5".to_string()), Some(super::catalog::DGVOODOO_URL.to_string()))
            }
        }
    };

    let update_avail = match (&current_ver, &latest_tag) {
        (Some(c), Some(l)) => is_newer_version(c, l),
        (None, Some(_)) => true,
        _ => false,
    };

    AddonVersionInfo {
        id,
        current_version: current_ver,
        latest_version: latest_tag,
        update_available: update_avail,
        download_url: dl_url,
    }
}

pub async fn check_all_addon_updates_force() -> Vec<AddonVersionInfo> {
    let mut results = Vec::new();
    for id in ALL_REFERENCED_ADDONS {
        results.push(check_addon_update(*id).await);
    }
    if let Ok(mut cache) = get_cache_mutex().lock() {
        *cache = Some((Instant::now(), results.clone()));
    }
    results
}

pub async fn check_all_addon_updates() -> Vec<AddonVersionInfo> {
    if let Ok(cache) = get_cache_mutex().lock() {
        if let Some((ts, ref list)) = *cache {
            if ts.elapsed() < VERSION_CACHE_TTL {
                return list.clone();
            }
        }
    }
    check_all_addon_updates_force().await
}

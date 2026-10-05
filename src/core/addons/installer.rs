use std::fs;
use std::path::{Path, PathBuf};
use crate::core::downloader::{
    download_file_with_progress, download_file_with_sha256, DownloadProgress, DownloadStage,
};
use super::catalog::*;
use super::cache::*;
use super::version::*;
use super::{AddonId, ALL_REFERENCED_ADDONS, DgVoodooComponents, FeederComponents};

/// Helper to execute tar silently on Windows without popping cmd/conhost consoles
fn silent_tar_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("tar");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Helper function to extract a ZIP archive safely using std::fs and zip crate
pub fn extract_zip(file: fs::File, out_dir: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip: {}", e))?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| format!("Zip entry error: {}", e))?;
        let outpath = match f.enclosed_name() {
            Some(path) => out_dir.join(path),
            None => continue,
        };
        if f.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| format!("Create dir error: {}", e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| format!("Create parent dir error: {}", e))?;
                }
            }
            let mut outfile = fs::File::create(&outpath).map_err(|e| format!("File create error: {}", e))?;
            std::io::copy(&mut f, &mut outfile).map_err(|e| format!("Extraction copy error: {}", e))?;
        }
    }
    Ok(())
}

/// Extracts ReShade32.dll and ReShade64.dll from the ReShade Add-on Setup archive
pub fn extract_reshade_from_setup(setup_path: &Path, comp_root: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(comp_root);
    let file = fs::File::open(setup_path).map_err(|e| format!("Failed to open ReShade setup: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Failed to open ReShade archive: {}", e))?;

    let reshade32_dest = comp_root.join("ReShade32.dll");
    let reshade64_dest = comp_root.join("ReShade64.dll");

    let reshade32_json = comp_root.join("ReShade32.json");
    let reshade64_json = comp_root.join("ReShade64.json");

    for i in 0..archive.len() {
        if let Ok(mut entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if name.ends_with("ReShade32.dll") {
                if let Ok(mut out) = fs::File::create(&reshade32_dest) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            } else if name.ends_with("ReShade64.dll") {
                if let Ok(mut out) = fs::File::create(&reshade64_dest) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            } else if name.ends_with("ReShade32.json") {
                if let Ok(mut out) = fs::File::create(&reshade32_json) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            } else if name.ends_with("ReShade64.json") {
                if let Ok(mut out) = fs::File::create(&reshade64_json) {
                    let _ = std::io::copy(&mut entry, &mut out);
                }
            }
        }
    }

    if reshade64_dest.is_file() {
        Ok(())
    } else {
        Err("ReShade64.dll could not be extracted from setup".to_string())
    }
}

/// Downloads, installs, or updates a single core add-on package
pub async fn install_or_update_addon<F>(
    id: AddonId,
    force: bool,
    mut progress_fn: F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let comp_root = get_components_root();
    let _ = fs::create_dir_all(&comp_root);

    let info = check_addon_update(id).await;
    let target_tag = info.latest_version.clone().unwrap_or_else(|| get_local_addon_version(id).unwrap_or_else(|| "latest".to_string()));
    
    crate::core::state::log_message(&format!("@{{log_addon_updating|{}|{}}}", id.name(), target_tag));

    match id {
        AddonId::RenoDxDlss5 => {
            if !force && is_renodx_engine_cached() {
                return Ok(());
            }
            let dl_url = info.download_url.unwrap_or_else(|| RENODX_DLSS5_URL.to_string());
            let renodx_dir = comp_root.join("renodx-dlss5");
            let _ = fs::create_dir_all(&renodx_dir);
            let target_dest = renodx_dir.join("renodx-dlss5.addon64");
            download_file_with_progress(
                &dl_url,
                &target_dest,
                "",
                &format!("RenoDX DLSS 5 ({})", target_tag),
                "renodx",
                &mut progress_fn,
            ).await?;
            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));
            let _ = record_addon_version(AddonId::RenoDxDlss5, &target_tag);
        }
        AddonId::DlssMipFix => {
            if !force && is_dlss5_d3d12_fix_cached() {
                return Ok(());
            }
            let fix_dir = comp_root.join("dlss-mip-fix");
            let _ = fs::create_dir_all(&fix_dir);
            let target_dest = fix_dir.join("dlss-mip-fix.addon64");

            // Check local build candidates first before downloading
            let local_candidates = [
                PathBuf::from("target/release/dlss_mip_addon.dll"),
                PathBuf::from("target/release/dlss-mip-fix.addon64"),
                PathBuf::from("target/debug/dlss_mip_addon.dll"),
                PathBuf::from("target/debug/dlss-mip-fix.addon64"),
                PathBuf::from("dist/dlss-mip-fix.addon64"),
                PathBuf::from("dist/dlss_mip_addon.dll"),
            ];
            let mut copied = false;
            for cand in &local_candidates {
                if cand.is_file() {
                    if fs::copy(cand, &target_dest).is_ok() {
                        copied = true;
                        break;
                    }
                }
            }

            if !copied {
                let dl_url = info.download_url.unwrap_or_else(|| DLSS_MIP_FIX_URL.to_string());
                download_file_with_progress(
                    &dl_url,
                    &target_dest,
                    "",
                    "DLSS Studio D3D12 Mip Companion",
                    "dlss_mip_fix",
                    &mut progress_fn,
                ).await?;
            }
            let _ = record_addon_version(AddonId::DlssMipFix, &target_tag);
        }
        AddonId::RenoDxMfgUnlock => {
            if !force && is_mfg_addon_cached() {
                return Ok(());
            }
            let dl_url = info.download_url.unwrap_or_else(|| MFG_UNLOCK_URL.to_string());
            let mfg_dir = comp_root.join("mfg-unlock-1.0");
            let _ = fs::create_dir_all(&mfg_dir);
            let target_dest = mfg_dir.join("renodx-mfgunlock.addon64");
            download_file_with_progress(
                &dl_url,
                &target_dest,
                "",
                &format!("RenoDX 4x MFG Unlock ({})", target_tag),
                "mfg_unlock",
                &mut progress_fn,
            ).await?;
            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));
            let _ = record_addon_version(AddonId::RenoDxMfgUnlock, &target_tag);
        }
        AddonId::Dlss5Feeder => {
            if !force && is_feeder_cached() {
                return Ok(());
            }
            let dl_url = info.download_url.unwrap_or_else(|| FEEDER_ARCHIVE_URL.to_string());
            let tag_clean = target_tag.trim_start_matches('v');
            let feeder_dir = comp_root.join(format!("DLSS5-Feeder-{}", tag_clean));
            let feeder_zip = comp_root.join(format!("DLSS5-Feeder-{}.zip", tag_clean));
            download_file_with_progress(
                &dl_url,
                &feeder_zip,
                "",
                &format!("DLSS 5 Feeder ({})", target_tag),
                "feeder",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));

            if let Ok(file) = fs::File::open(&feeder_zip) {
                let feeder_dir_c = feeder_dir.clone();
                let feeder_zip_c = feeder_zip.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &feeder_dir_c);
                    let _ = fs::remove_file(&feeder_zip_c);
                }).await;
            }

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), target_tag));

            // Purge older DLSS5-Feeder-* folders from comp_root to prevent stale accumulation
            if let Ok(entries) = fs::read_dir(&comp_root) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("DLSS5-Feeder-") && p != feeder_dir {
                            if fs::remove_dir_all(&p).is_ok() {
                                crate::core::state::log_message(&format!("@{{log_addon_cache_cleaned|{}}}", name));
                            }
                        }
                    }
                }
            }

            let mut dummy_log = Vec::new();
            ensure_feeder_components(&mut dummy_log).await?;
            let _ = record_addon_version(AddonId::Dlss5Feeder, &target_tag);
        }
        AddonId::Rtx40Mfg => {
            if !force && is_rtxmfg_cached() {
                return Ok(());
            }
            let (dl_url, tag) = resolve_latest_dashdogy_mfg().await;
            let mfg_dir = comp_root.join("mfg-standalone");
            let _ = fs::create_dir_all(&mfg_dir);
            let zip_path = comp_root.join(format!("RTXMFG-{}.zip", tag));

            download_file_with_progress(
                &dl_url,
                &zip_path,
                "",
                &format!("Universal RTX40MFG-Unlock ({})", tag),
                "rtxmfg",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), tag));

            if let Ok(file) = fs::File::open(&zip_path) {
                let mfg_dir_c = mfg_dir.clone();
                let zip_path_c = zip_path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &mfg_dir_c);
                    let _ = fs::remove_file(&zip_path_c);
                }).await;
            }

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), tag));
            let _ = record_addon_version(AddonId::Rtx40Mfg, &tag);
        }
        AddonId::OptiScaler => {
            if !force && is_optiscaler_cached() {
                return Ok(());
            }
            let (dl_url, tag) = resolve_latest_optiscaler_mfg().await;
            let tag_clean = tag.trim_start_matches('v');
            let opti_dir = comp_root.join(format!("OptiScaler-{}-rtx40-mfg", tag_clean));
            let zip_path = comp_root.join("OptiScaler-latest.zip");

            download_file_with_progress(
                &dl_url,
                &zip_path,
                "",
                &format!("OptiScaler DLSS-NR ({})", tag),
                "optiscaler",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), tag));

            if let Ok(file) = fs::File::open(&zip_path) {
                let opti_dir_c = opti_dir.clone();
                let zip_path_c = zip_path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &opti_dir_c);
                    let _ = fs::remove_file(&zip_path_c);
                }).await;
            }

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), tag));

            // Clean older OptiScaler-* directories from comp_root
            if let Ok(entries) = fs::read_dir(&comp_root) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("OptiScaler-") && p != opti_dir {
                            if fs::remove_dir_all(&p).is_ok() {
                                crate::core::state::log_message(&format!("@{{log_addon_cache_cleaned|{}}}", name));
                            }
                        }
                    }
                }
            }

            let _ = record_addon_version(AddonId::OptiScaler, &tag);
        }
        AddonId::ReShade => {
            if !force && is_reshade_cached() {
                return Ok(());
            }
            let setup_path = comp_root.join("ReShade_Setup_6.8.0_Addon.exe");
            download_file_with_progress(
                RESHADE_SETUP_URL,
                &setup_path,
                RESHADE_SETUP_SHA256,
                "ReShade 6.8.0 (Add-on Runtime)",
                "reshade",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), "v6.8.0"));

            let setup_path_c = setup_path.clone();
            let comp_root_c = comp_root.clone();
            let extract_res = tokio::task::spawn_blocking(move || {
                let res = extract_reshade_from_setup(&setup_path_c, &comp_root_c);
                let _ = fs::remove_file(&setup_path_c);
                res
            }).await.map_err(|e| format!("ReShade extraction task error: {}", e))?;

            extract_res?;
            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), "v6.8.0"));
            let _ = record_addon_version(AddonId::ReShade, "v6.8.0");
        }
        AddonId::Streamline => {
            if !force && is_streamline_cached() {
                return Ok(());
            }
            let streamline_dir = comp_root.join("streamline-2.14.1");
            let zip_path = comp_root.join("streamline.zip");
            download_file_with_progress(
                STREAMLINE_ZIP_URL,
                &zip_path,
                "",
                "Streamline Runtime v2.14.1",
                "streamline",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), "v2.14.1"));

            if let Ok(file) = fs::File::open(&zip_path) {
                let streamline_dir_c = streamline_dir.clone();
                let zip_path_c = zip_path.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &streamline_dir_c);
                    let _ = fs::remove_file(&zip_path_c);
                }).await;
            }

            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), "v2.14.1"));
            let _ = record_addon_version(AddonId::Streamline, "v2.14.1");
        }
        AddonId::DgVoodoo => {
            if !force && is_dgvoodoo_cached() {
                return Ok(());
            }
            let dl_url = info.download_url.unwrap_or_else(|| DGVOODOO_URL.to_string());
            let zip_path = comp_root.join("dgVoodoo2_87_5.zip");
            download_file_with_progress(
                &dl_url,
                &zip_path,
                "",
                &format!("dgVoodoo2 ({})", target_tag),
                "dgvoodoo",
                &mut progress_fn,
            ).await?;

            crate::core::state::log_message(&format!("@{{log_addon_download_success|{}|{}}}", id.name(), target_tag));

            let comp_root_c = comp_root.clone();
            let zip_path_c = zip_path.clone();
            let extract_res = tokio::task::spawn_blocking(move || {
                let dgvoodoo_dir = comp_root_c.join("dgvoodoo");
                let _ = fs::create_dir_all(dgvoodoo_dir.join("x86"));
                let _ = fs::create_dir_all(dgvoodoo_dir.join("x64"));

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(&dgvoodoo_dir)
                    .arg("dgVoodoo.conf")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x86"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x86/D3D9.dll")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x86"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x86/D3D8.dll")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x64"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x64/D3D9.dll")
                    .status();

                let _ = fs::remove_file(&zip_path_c);
            }).await;

            if let Err(e) = extract_res {
                return Err(format!("dgVoodoo2 extraction error: {}", e));
            }
            crate::core::state::log_message(&format!("@{{log_addon_unpack_success|{}|{}}}", id.name(), target_tag));
            let _ = record_addon_version(AddonId::DgVoodoo, &target_tag);
        }
    }

    invalidate_addon_cache();
    crate::core::state::log_message(&format!("@{{log_addon_updated|{}|{}}}", id.name(), target_tag));
    Ok(())
}

/// Helper to update a single addon
pub async fn update_single_addon<F>(id: AddonId, progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    install_or_update_addon(id, true, progress_fn).await
}

/// Helper to update multiple addons in sequence with aggregated progress
pub async fn update_all_addons<F>(ids: &[AddonId], mut progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let total = ids.len() as f32;
    if total == 0.0 {
        return Ok(());
    }

    for (idx, &id) in ids.iter().enumerate() {
        let step_offset = idx as f32;
        let mut step_progress = |mut p: DownloadProgress| {
            p.percentage = ((step_offset * 100.0) + p.percentage) / total;
            progress_fn(p);
        };
        install_or_update_addon(id, true, &mut step_progress).await?;
    }

    Ok(())
}

/// Sequentially checks and ensures all mandatory components are cached on disk
pub async fn ensure_mandatory_components_cached_with_progress<F>(mut progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let total_steps = ALL_REFERENCED_ADDONS.len() as f32;
    let mut errors = Vec::new();

    for (idx, &id) in ALL_REFERENCED_ADDONS.iter().enumerate() {
        if is_addon_cached(id) {
            continue;
        }

        let step_offset = idx as f32;
        let mut step_progress = |mut p: DownloadProgress| {
            p.percentage = ((step_offset * 100.0) + p.percentage) / total_steps;
            progress_fn(p);
        };

        if let Err(e) = install_or_update_addon(id, false, &mut step_progress).await {
            crate::core::state::log_message(&format!("@{{log_download_error|{:?}|{}}}", id, e));
            errors.push(format!("{:?}: {}", id, e));
        }
    }

    if are_all_mandatory_components_cached() {
        progress_fn(DownloadProgress {
            is_downloading: false,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 100.0,
            stage: DownloadStage::Downloading,
            message: "All mandatory components ready".to_string(),
        });
        Ok(())
    } else {
        let err_msg = format!("Failed to download some mandatory components: {}", errors.join(", "));
        progress_fn(DownloadProgress {
            is_downloading: false,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            stage: DownloadStage::Downloading,
            message: err_msg.clone(),
        });
        Err(err_msg)
    }
}

/// Helper for backwards compatibility across existing callers
pub async fn ensure_all_mandatory_components_with_progress<F>(progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    ensure_mandatory_components_cached_with_progress(progress_fn).await
}

/// Helper to ensure DLSS Mip Fix companion addon is cached
pub async fn ensure_dlss5_d3d12_fix_addon(_log: &mut Vec<String>) -> Result<PathBuf, String> {
    install_or_update_addon(AddonId::DlssMipFix, false, |_| {}).await?;
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        let p = root.join("dlss-mip-fix").join("dlss-mip-fix.addon64");
        if p.is_file() {
            return Ok(p);
        }
        let p2 = root.join("dlss-mip-fix.addon64");
        if p2.is_file() {
            return Ok(p2);
        }
    }
    Err("DLSS Mip Fix addon could not be located".to_string())
}

/// Helper to ensure RenoDX MFG Unlock addon is cached
pub async fn ensure_mfg_v09_addon(_log: &mut Vec<String>) -> Result<PathBuf, String> {
    install_or_update_addon(AddonId::RenoDxMfgUnlock, false, |_| {}).await?;
    let mut roots = vec![get_components_root()];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.join("components"));
        }
    }
    for root in roots {
        let p = root.join("mfg-unlock-1.0").join("renodx-mfgunlock.addon64");
        if p.is_file() {
            return Ok(p);
        }
        let p2 = root.join("renodx-mfgunlock.addon64");
        if p2.is_file() {
            return Ok(p2);
        }
    }
    Err("RenoDX MFG Unlock addon could not be located".to_string())
}

/// Helper for backwards compatibility
pub async fn ensure_feeder_components(log: &mut Vec<String>) -> Result<FeederComponents, String> {
    if let Some(local) = find_local_feeder_components() {
        log.push(format!("[FEEDER-CACHE] Using verified local feeder components from {}", local.addon64.parent().unwrap_or(&local.addon64).display()));
        return Ok(local);
    }

    let comp_root = get_components_root();
    let info = check_addon_update(AddonId::Dlss5Feeder).await;
    let dl_url = info.download_url.unwrap_or_else(|| FEEDER_ARCHIVE_URL.to_string());
    let tag = info.latest_version.unwrap_or_else(|| "1.16.0-beta.3".to_string());
    let tag_clean = tag.trim_start_matches('v');
    let feeder_dir = comp_root.join(format!("DLSS5-Feeder-{}", tag_clean));
    let feeder_zip = comp_root.join(format!("DLSS5-Feeder-{}.zip", tag_clean));

    // 1. Download & extract DLSS5-Feeder
    if !feeder_dir.join("dlss5-feed.addon64").is_file() {
        log.push(format!("[DOWNLOAD] Fetching latest DLSS5-Feeder ({}) from upstream GitHub...", tag));
        download_file_with_progress(&dl_url, &feeder_zip, "", &format!("DLSS 5 Feeder ({})", tag), "feeder", |_| {}).await?;
        log.push("[DOWNLOAD] DLSS5-Feeder downloaded successfully".to_string());

        let file = fs::File::open(&feeder_zip).map_err(|e| format!("Failed to open {}: {}", feeder_zip.display(), e))?;
        extract_zip(file, &feeder_dir)?;
        let _ = fs::remove_file(&feeder_zip);
        let _ = record_addon_version(AddonId::Dlss5Feeder, &tag);
        log.push("[FEEDER] DLSS5-Feeder unpacked successfully".to_string());
    }

    // 2. Download & extract vort_Shaders
    let vort_dir = comp_root.join("vort_Shaders-b410b9f");
    let vort_zip = comp_root.join("vort_Shaders-b410b9f.zip");
    if !vort_dir.join("Shaders").join("vort_Motion.fx").is_file() {
        log.push("[DOWNLOAD] Fetching latest VORT Motion Vector shaders from upstream...".to_string());
        download_file_with_sha256(VORT_ARCHIVE_URL, &vort_zip, VORT_ARCHIVE_SHA256).await?;
        log.push("[DOWNLOAD] Verifying VORT Shaders SHA-256 checksum: OK".to_string());

        let file = fs::File::open(&vort_zip).map_err(|e| format!("Failed to open {}: {}", vort_zip.display(), e))?;
        extract_zip(file, &vort_dir)?;
        let _ = fs::remove_file(&vort_zip);
    }

    // 3. Download ReShade standard framework headers and textures
    let slim_dir = comp_root.join("reshade-shaders-slim");
    let slim_zip = comp_root.join("reshade-shaders-slim.zip");
    if !slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc").join("Shaders").join("DrawText.fxh").is_file()
        && !slim_dir.join("Shaders").join("DrawText.fxh").is_file()
    {
        log.push("[DOWNLOAD] Fetching ReShade framework headers and textures from upstream slim branch...".to_string());
        if download_file_with_sha256(RESHADE_SHADERS_SLIM_URL, &slim_zip, RESHADE_SHADERS_SLIM_SHA256).await.is_ok() {
            if let Ok(file) = fs::File::open(&slim_zip) {
                let _ = extract_zip(file, &slim_dir);
                let _ = fs::remove_file(&slim_zip);
                log.push("[FEEDER] ReShade framework headers unpacked successfully".to_string());
            }
        }
    }

    let slim_sub = if slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc").is_dir() {
        slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc")
    } else {
        slim_dir.clone()
    };

    // 4. Assemble consolidated reshade-shaders tree with full parity to v2.0.1
    let consolidated_shaders = comp_root.join("feeder-shaders");
    let target_shaders = consolidated_shaders.join("Shaders");
    let target_textures = consolidated_shaders.join("Textures");
    let target_includes = target_shaders.join("Includes");
    fs::create_dir_all(&target_includes).map_err(|e| format!("Failed to create {}: {}", target_includes.display(), e))?;
    fs::create_dir_all(&target_textures).map_err(|e| format!("Failed to create {}: {}", target_textures.display(), e))?;

    // Copy DLSS5_Feed.fx from feeder
    let dlss5_feed_fx = feeder_dir.join("reshade-shaders").join("Shaders").join("DLSS5_Feed.fx");
    if dlss5_feed_fx.is_file() {
        let _ = fs::copy(&dlss5_feed_fx, target_shaders.join("DLSS5_Feed.fx"));
    }

    // Copy all framework headers and textures from slim bundle
    if slim_sub.join("Shaders").is_dir() {
        for entry in walkdir::WalkDir::new(slim_sub.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".fxh") || fname.ends_with(".fx") {
                    let _ = fs::copy(entry.path(), target_shaders.join(&fname));
                }
            }
        }
    }
    if slim_sub.join("Textures").is_dir() {
        for entry in walkdir::WalkDir::new(slim_sub.join("Textures")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let _ = fs::copy(entry.path(), target_textures.join(fname));
            }
        }
    }

    // Fallback direct header copies
    let headers_dir = comp_root.join("reshade-headers");
    let fxh_path = headers_dir.join("ReShade.fxh");
    let ui_fxh_path = headers_dir.join("ReShadeUI.fxh");
    let drawtext_path = headers_dir.join("DrawText.fxh");
    let fontatlas_path = headers_dir.join("FontAtlas.png");

    if fxh_path.is_file() {
        let _ = fs::copy(&fxh_path, target_shaders.join("ReShade.fxh"));
    }
    if ui_fxh_path.is_file() {
        let _ = fs::copy(&ui_fxh_path, target_shaders.join("ReShadeUI.fxh"));
    }
    if drawtext_path.is_file() {
        let _ = fs::copy(&drawtext_path, target_shaders.join("DrawText.fxh"));
    }
    if fontatlas_path.is_file() {
        let _ = fs::copy(&fontatlas_path, target_textures.join("FontAtlas.png"));
    }

    // Copy VORT motion shaders and includes
    let vort_sub = if vort_dir.join("vort_Shaders-b410b9f0c0fbb83c8cb42164aaf1655fab386f4a").is_dir() {
        vort_dir.join("vort_Shaders-b410b9f0c0fbb83c8cb42164aaf1655fab386f4a")
    } else {
        vort_dir.clone()
    };

    if vort_sub.join("Shaders").is_dir() {
        for entry in walkdir::WalkDir::new(vort_sub.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".fx") || fname.ends_with(".fxh") {
                    let dest = if entry.path().parent().map(|p| p.ends_with("Includes")).unwrap_or(false) {
                        target_includes.join(&fname)
                    } else {
                        target_shaders.join(&fname)
                    };
                    let _ = fs::copy(entry.path(), dest);
                }
            }
        }
    }

    // Restores vort_BlueNoise.png and all noise textures
    if vort_sub.join("Textures").is_dir() {
        for entry in walkdir::WalkDir::new(vort_sub.join("Textures")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let _ = fs::copy(entry.path(), target_textures.join(fname));
            }
        }
    }

    find_local_feeder_components().ok_or_else(|| "DLSS 5 Feeder components could not be located after download".to_string())
}

/// Helper for backwards compatibility
pub async fn ensure_dgvoodoo_components(_log: &mut Vec<String>) -> Result<DgVoodooComponents, String> {
    if let Some(local) = find_local_dgvoodoo_components() {
        return Ok(local);
    }
    install_or_update_addon(AddonId::DgVoodoo, false, |_| {}).await?;
    find_local_dgvoodoo_components().ok_or_else(|| "dgVoodoo2 components could not be located after download".to_string())
}

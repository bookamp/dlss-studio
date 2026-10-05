#![allow(dead_code)]

use std::fs;
use std::path::Path;
use crate::core::journal::{ActiveManifest, ManifestGame, save_manifest, append_history, HistoryRow};
use crate::core::payloads::PayloadBundle;
use crate::core::ini::set_ini;
use crate::core::scan::GameEntry;
use crate::core::advisories::RouteAdvisory;
use super::common::{
    DeployOptions, DeployResult, clean_conflicting_route_artifacts,
    remove_stale_proxy_hooks, carry_forward_existing_backups, track_and_copy, track_and_write,
};
use super::Route;

#[derive(Debug, Clone)]
pub struct OptiScalerOptions {
    pub pre_sr: bool,
    pub passes: u32,
    pub mfg_unlock: bool,
    pub target_exe_name: String,
    pub nr_style: usize,
    pub api: String,
    pub mfg_multiplier: u32,
}

impl Default for OptiScalerOptions {
    fn default() -> Self {
        Self {
            pre_sr: true,
            passes: 3,
            mfg_unlock: false,
            target_exe_name: String::new(),
            nr_style: 0,
            api: "DirectX 12".to_string(),
            mfg_multiplier: 4,
        }
    }
}

pub fn generate_optiscaler_ini(
    base_text: &str,
    pre_sr: bool,
    passes: u32,
    external_mfg: bool,
    target_exe: Option<&str>,
    nr_style: usize,
    api: Option<&str>,
    mfg_multiplier: u32,
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

    if external_mfg {
        text = set_ini(&text, "FrameGen", "Enabled", "true");
        let is_dx11 = api.map(|a| a.to_lowercase().contains("11")).unwrap_or(false);
        if is_dx11 {
            text = set_ini(&text, "FrameGen", "FGInput", "upscaler");
            text = set_ini(&text, "FrameGen", "FGOutput", "dlssg");
            text = set_ini(&text, "FrameGen", "FGNvngxReplacement", "None");
            text = set_ini(&text, "OptiFG", "HUDFix", "true");
            text = set_ini(&text, "Dx11withDx12", "BuiltinMfgUnlock", "true");
            text = set_ini(&text, "Dx11withDx12", "UseDelayedInit", "true");
        } else {
            text = set_ini(&text, "FrameGen", "FGInput", "auto");
            text = set_ini(&text, "FrameGen", "FGOutput", "auto");
            text = set_ini(&text, "Dx11withDx12", "BuiltinMfgUnlock", "true");
        }
        text = set_ini(&text, "MfgUnlock", "Enabled", "true");
        text = set_ini(&text, "MfgUnlock", "Multiplier", &mfg_multiplier.to_string());
        text = set_ini(&text, "DLSSG", "Multiplier", &mfg_multiplier.to_string());
        text = set_ini(&text, "DLSSG", "AdaMfgUnlock", "true");
        let interp_count = if mfg_multiplier >= 2 { mfg_multiplier - 1 } else { 1 };
        text = set_ini(&text, "DLSSG", "InterpolationCount", &interp_count.to_string());
        text = set_ini(&text, "DLSSG", "OverrideInterpolationCount", "true");
    } else {
        text = set_ini(&text, "FrameGen", "Enabled", "false");
        text = set_ini(&text, "Dx11withDx12", "BuiltinMfgUnlock", "false");
        text = set_ini(&text, "MfgUnlock", "Enabled", "false");
        text = set_ini(&text, "MfgUnlock", "Multiplier", "auto");
        text = set_ini(&text, "DLSSG", "Multiplier", "auto");
        text = set_ini(&text, "DLSSG", "AdaMfgUnlock", "false");
        text = set_ini(&text, "DLSSG", "InterpolationCount", "auto");
        text = set_ini(&text, "DLSSG", "OverrideInterpolationCount", "auto");
    }

    text = set_ini(&text, "Menu", "OverlayMenu", "true");
    text = set_ini(&text, "Menu", "ShortcutKey", "0x2D"); // INSERT key

    if let Some(api_str) = api {
        let api_lower = api_str.to_lowercase();
        if api_lower.contains("11") {
            text = set_ini(&text, "Upscalers", "Dx11Upscaler", "dlss_12");
        } else if api_lower.contains("vulkan") {
            text = set_ini(&text, "Upscalers", "VulkanUpscaler", "dlss");
        } else {
            text = set_ini(&text, "Upscalers", "Dx12Upscaler", "dlss");
        }
    }

    text = set_ini(&text, "Spoofing", "StreamlineSpoofing", "false");
    text = set_ini(&text, "Spoofing", "Dxgi", "false");

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
        Some(&opts.api),
        opts.mfg_multiplier,
    )
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
        api: opts.api.clone(),
        mfg_multiplier: opts.mfg_multiplier,
    });
    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &mod_root.join("OptiScaler.ini"), &configured_ini, "config", &mut log)
        .map_err(|e| format!("Failed to write OptiScaler.ini: {}", e))?;

    // 5. Deploy Standalone 4x MFG (Universal RTXMFG) as version.dll
    if opts.mfg_unlock {
        if opts.api == "Vulkan" {
            log.push("[MFG] Vulkan route active - standalone DirectX 12 RTXMFG hook bypassed".to_string());
        } else if let Some(mfg_dll) = &payloads.rtxmfg_dll {
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
    } else if opts.mfg_unlock {
        if let Some(streamline_src) = &payloads.streamline_dir {
            if streamline_src.is_dir() {
                let sl_files = [
                    "sl.interposer.dll",
                    "sl.common.dll",
                    "sl.dlss_g.dll",
                    "sl.reflex.dll",
                    "sl.pcl.dll",
                    "nvngx_dlssg.dll",
                ];
                let opti_sl_dir = mod_root.join("OptiScaler").join("streamline");
                for f in &sl_files {
                    let src_file = streamline_src.join(f);
                    if src_file.is_file() {
                        let dest_file = opti_sl_dir.join(f);
                        track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, &src_file, &dest_file, "streamline", &mut log)
                            .map_err(|e| format!("Failed to deploy OptiScaler streamline runtime {}: {}", f, e))?;
                    }
                }
                let dlssg_src = streamline_src.join("nvngx_dlssg.dll");
                if dlssg_src.is_file() {
                    let dest_file = mod_root.join("nvngx_dlssg.dll");
                    track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, &dlssg_src, &dest_file, "streamline", &mut log)
                        .map_err(|e| format!("Failed to deploy nvngx_dlssg.dll to mod root: {}", e))?;
                }
                log.push("[STREAMLINE] Deployed verified Streamline 2.14.1 stack into OptiScaler/streamline for DLSS-G FrameGen".to_string());
            }
        }
    }

    // 7. Save manifest and history
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

pub fn deploy_optiscaler(opts: &DeployOptions) -> Result<DeployResult, String> {
    let payloads = PayloadBundle::from_system()?;
    deploy_optiscaler_with_bundle(opts, &payloads)
}

/// Pluggable Route implementation for OptiScaler DLSS-NR.
#[derive(Debug, Default, Clone, Copy)]
pub struct OptiScalerRoute;

impl Route for OptiScalerRoute {
    fn id(&self) -> &'static str {
        "optiscaler"
    }

    fn display_name(&self) -> &'static str {
        "OptiScaler DLSS-NR"
    }

    fn backend(&self) -> &'static str {
        "optiscaler"
    }

    fn evaluate_advisory(&self, game: &GameEntry) -> Option<RouteAdvisory> {
        crate::core::advisories::get_optiscaler_advisory(game)
    }

    fn deploy(&self, opts: &DeployOptions, payloads: &PayloadBundle) -> Result<DeployResult, String> {
        deploy_optiscaler_with_bundle(opts, payloads)
    }
}

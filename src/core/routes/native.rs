#![allow(dead_code)]

use std::fs;
use std::path::PathBuf;
use crate::core::journal::{ActiveManifest, ManifestGame, save_manifest, append_history, HistoryRow};
use crate::core::payloads::PayloadBundle;
use crate::core::scan::GameEntry;
use crate::core::advisories::RouteAdvisory;
use super::common::{
    DeployOptions, DeployResult, clean_conflicting_route_artifacts,
    remove_stale_proxy_hooks, carry_forward_existing_backups, track_and_copy, track_and_write,
};
use crate::core::ini::{set_ini, get_ini, configure_mfg_unlock_ini};
use super::Route;

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

    // 2. Deploy DLSS 5 Neural Rendering engine (dlss-nr, renodx-dlss, or renodx-dlss5) if active
    let mut deployed_addon_stems: Vec<String> = Vec::new();
    if renodx_active {
        if let Some(renodx_src) = &payloads.renodx_dlss5_addon {
            if renodx_src.is_file() {
                let file_name = renodx_src.file_name().and_then(|n| n.to_str()).unwrap_or("dlss-nr.addon64");
                let dest = mod_root.join(file_name);
                track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, renodx_src, &dest, "addon", &mut log)
                    .map_err(|e| format!("Failed to copy {}: {}", file_name, e))?;
                let stem = renodx_src.file_stem().and_then(|s| s.to_str()).unwrap_or("dlss-nr").to_string();
                deployed_addon_stems.push(stem.clone());
                log.push(format!("[ADDON] {} deployed (DLSS 5 Neural Rendering Engine)", file_name));

                // Clean up any conflicting other neural rendering addons so only one is active
                let conflicting = ["dlss-nr.addon64", "renodx-dlss.addon64", "renodx-dlss5.addon64"];
                for c in &conflicting {
                    if *c != file_name {
                        let stale = mod_root.join(c);
                        if stale.is_file() {
                            let _ = fs::remove_file(&stale);
                            log.push(format!("[CLEAN] Removed conflicting add-on {}", c));
                        }
                    }
                }
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

    // Pure Rust DLSS 5 Neural Rendering Stacker configuration
    configured_reshade_ini = set_ini(&configured_reshade_ini, "DLSS_NR", "Passes", "1");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "DLSS_NR", "ResolutionMode", "0");
    configured_reshade_ini = set_ini(&configured_reshade_ini, "DLSS_NR", "PreSR", "0");

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

/// Pluggable Route implementation for Native RenoDX DLSS 5 Direct.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeRoute;

impl Route for NativeRoute {
    fn id(&self) -> &'static str {
        "native"
    }

    fn display_name(&self) -> &'static str {
        "DLSS 5 Direct (RenoDX)"
    }

    fn backend(&self) -> &'static str {
        "reshade"
    }

    fn evaluate_advisory(&self, game: &GameEntry) -> Option<RouteAdvisory> {
        crate::core::advisories::get_native_dlss_advisory(game)
    }

    fn deploy(&self, opts: &DeployOptions, payloads: &PayloadBundle) -> Result<DeployResult, String> {
        deploy_native_dlss5_with_bundle(opts, payloads)
    }
}

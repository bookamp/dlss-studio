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
    let preset_path = mod_root.join("ReShadePreset.ini");
    let existing_preset = fs::read_to_string(&preset_path).unwrap_or_default();
    let preset_content = configure_feeder_preset(&existing_preset);
    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &preset_path, &preset_content, "config", &mut log)
        .map_err(|e| format!("Failed to write ReShadePreset.ini: {}", e))?;

    // 3. Deploy RenoDX DLSS 5 Engine & Neural Rendering runtime
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

    // Deploy / upgrade modern nvngx_dlss.dll runtime
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

    // 3b. For 32-bit games, assemble the host64 companion directory
    if bitness == 32 {
        if let Some(fc) = &payloads.feeder_components {
            if let Some(host64_exe) = &fc.host64 {
                if host64_exe.is_file() {
                    let host64_dir = mod_root.join("host64");

                    let dest_host_exe = host64_dir.join("dlss5-feed-host64.exe");
                    track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, host64_exe, &dest_host_exe, "feeder-host", &mut log)
                        .map_err(|e| format!("Failed to copy dlss5-feed-host64.exe: {}", e))?;

                    if let Some(reshade64) = &payloads.reshade64_dll {
                        if reshade64.is_file() {
                            let dest_r64 = host64_dir.join("dxgi.dll");
                            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, reshade64, &dest_r64, "feeder-host-reshade", &mut log)
                                .map_err(|e| format!("Failed to copy ReShade64 to host64/dxgi.dll: {}", e))?;
                        }
                    }

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

                    if let Some(dlss) = &payloads.nvngx_dlss_dll {
                        if dlss.is_file() {
                            let dest_dlss = host64_dir.join("nvngx_dlss.dll");
                            track_and_copy(&mut manifest, &opts.game_dir, &backup_dir, dlss, &dest_dlss, "feeder-host-runtime", &mut log)
                                .map_err(|e| format!("Failed to copy nvngx_dlss.dll to host64: {}", e))?;
                        }
                    }

                    let host_reshade_ini = configure_host64_reshade_ini(opts.nr_style);
                    let host_ini_path = host64_dir.join("ReShade.ini");
                    track_and_write(&mut manifest, &opts.game_dir, &backup_dir, &host_ini_path, &host_reshade_ini, "config", &mut log)
                        .map_err(|e| format!("Failed to write host64/ReShade.ini: {}", e))?;

                    log.push("[HOST64] Assembled 64-bit Neural Host bridge (host64/) for 32-bit game".to_string());
                }
            }
        }
    }

    // 4. Deploy Streamline Feeder addons when MFG is enabled
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

    // 6. Save manifest and history
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
                    handle.block_on(crate::core::addons::ensure_feeder_components(&mut download_log))
                })
            }
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("Tokio runtime error: {}", e))?;
                rt.block_on(crate::core::addons::ensure_feeder_components(&mut download_log))
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
                    handle.block_on(crate::core::addons::ensure_dgvoodoo_components(&mut download_log))
                })
            }
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("Tokio runtime error: {}", e))?;
                rt.block_on(crate::core::addons::ensure_dgvoodoo_components(&mut download_log))
            }
        }?;
        payloads.dgvoodoo = Some(dg);
    }

    deploy_feeder_with_bundle(opts, &payloads)
}

/// Pluggable Route implementation for DLSS 5 Feeder.
#[derive(Debug, Default, Clone, Copy)]
pub struct FeederRoute;

impl Route for FeederRoute {
    fn id(&self) -> &'static str {
        "feeder"
    }

    fn display_name(&self) -> &'static str {
        "DLSS 5 Feeder"
    }

    fn backend(&self) -> &'static str {
        "reshade"
    }

    fn evaluate_advisory(&self, _game: &GameEntry) -> Option<RouteAdvisory> {
        None
    }

    fn deploy(&self, opts: &DeployOptions, payloads: &PayloadBundle) -> Result<DeployResult, String> {
        deploy_feeder_with_bundle(opts, payloads)
    }
}

use dlss_studio::core::optiscaler::*;
use dlss_studio::core::state::STATE_TEST_MUTEX;
use crate::common::TempDir;
use std::fs;
use std::path::Path;

fn create_mock_pe32() -> Vec<u8> {
    let mut data = vec![0u8; 1024];
    data[0..2].copy_from_slice(b"MZ");
    data[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    data[0x80..0x84].copy_from_slice(b"PE\0\0");
    data[0x84..0x86].copy_from_slice(&0x014Cu16.to_le_bytes());
    data[0x94..0x96].copy_from_slice(&0xE0u16.to_le_bytes());
    data[0x98..0x9A].copy_from_slice(&0x010Bu16.to_le_bytes());
    data
}

fn create_mock_pe64() -> Vec<u8> {
    let mut data = vec![0u8; 1024];
    data[0..2].copy_from_slice(b"MZ");
    data[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    data[0x80..0x84].copy_from_slice(b"PE\0\0");
    data[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes()); // IMAGE_FILE_MACHINE_AMD64
    data[0x94..0x96].copy_from_slice(&0xF0u16.to_le_bytes()); // SizeOfOptionalHeader
    data[0x98..0x9A].copy_from_slice(&0x020Bu16.to_le_bytes()); // PE32+ (64-bit) magic
    data
}

fn create_mock_payload_bundle(temp: &Path) -> PayloadBundle {
    let opti_dir = temp.join("mock_opti_payload");
    fs::create_dir_all(opti_dir.join("OptiScaler")).unwrap();
    let optiscaler_dll = opti_dir.join("OptiScaler.dll");
    fs::write(&optiscaler_dll, b"MOCK_OPTISCALER_PE_BYTES").unwrap();
    let optiscaler_ini = opti_dir.join("OptiScaler.ini");
    fs::write(&optiscaler_ini, b"[DlssNr]\nEnabled=false\n\n[Plugins]\nLoadReshade=true\n\n[FrameGen]\nExternal=false\n").unwrap();

    let nr_dll = temp.join("nvngx_dlssnr.dll");
    fs::write(&nr_dll, b"MOCK_DLSSNR_PE_BYTES").unwrap();

    let rtxmfg = temp.join("RTXMFG.dll");
    fs::write(&rtxmfg, b"MOCK_RTXMFG_PE_BYTES").unwrap();

    let reshade = temp.join("ReShade64.dll");
    fs::write(&reshade, b"MOCK_RESHADE_PE_BYTES").unwrap();

    let reshade32 = temp.join("ReShade32.dll");
    fs::write(&reshade32, b"MOCK_RESHADE32_PE_BYTES").unwrap();

    let renodx = temp.join("renodx-dlss5.addon64");
    fs::write(&renodx, b"MOCK_RENODX_PE_BYTES").unwrap();

    let fix = temp.join("dlss5-d3d12-fix.addon64");
    fs::write(&fix, b"MOCK_D3D12_FIX_PE_BYTES").unwrap();

    let mfgunlock = temp.join("renodx-mfgunlock.addon64");
    fs::write(&mfgunlock, b"MOCK_MFGUNLOCK_PE_BYTES").unwrap();

    let snippet = temp.join("nvngx.dll_dlssnr.dll");
    fs::write(&snippet, b"MOCK_SNIPPET_PE_BYTES").unwrap();

    let streamline_dir = temp.join("mock_streamline");
    fs::create_dir_all(&streamline_dir).unwrap();
    fs::write(streamline_dir.join("sl.interposer.dll"), b"MOCK_SL_INTERPOSER").unwrap();
    fs::write(streamline_dir.join("sl.common.dll"), b"MOCK_SL_COMMON").unwrap();
    fs::write(streamline_dir.join("sl.dlss_g.dll"), b"MOCK_SL_DLSSG").unwrap();
    fs::write(streamline_dir.join("sl.reflex.dll"), b"MOCK_SL_REFLEX").unwrap();
    fs::write(streamline_dir.join("sl.pcl.dll"), b"MOCK_SL_PCL").unwrap();
    fs::write(streamline_dir.join("nvngx_dlssg.dll"), b"MOCK_NVNGX_DLSSG").unwrap();

    let feeder_dir = temp.join("mock_feeder");
    let feeder_shaders = feeder_dir.join("feeder-shaders");
    fs::create_dir_all(feeder_shaders.join("Shaders")).unwrap();
    fs::create_dir_all(feeder_shaders.join("Textures")).unwrap();
    fs::write(feeder_shaders.join("Shaders").join("DLSS5_Feed.fx"), b"// mock feed").unwrap();
    fs::write(feeder_shaders.join("Shaders").join("vort_Motion.fx"), b"// mock motion").unwrap();
    fs::write(feeder_shaders.join("Shaders").join("DrawText.fxh"), b"// mock drawtext").unwrap();
    fs::write(feeder_shaders.join("Textures").join("FontAtlas.png"), b"MOCK_PNG").unwrap();
    let mock_addon64 = feeder_dir.join("dlss5-feed.addon64");
    fs::write(&mock_addon64, b"MOCK_FEEDER_ADDON_64").unwrap();

    let mock_addon32 = feeder_dir.join("dlss5-feed.addon32");
    fs::write(&mock_addon32, b"MOCK_FEEDER_ADDON_32").unwrap();

    let mock_host64 = feeder_dir.join("dlss5-feed-host64.exe");
    fs::write(&mock_host64, b"MOCK_FEEDER_HOST64_EXE").unwrap();

    let mock_feeder = dlss_studio::core::downloader::FeederComponents {
        addon64: mock_addon64,
        addon32: Some(mock_addon32),
        host64: Some(mock_host64),
        shader_dir: feeder_shaders,
        vk_layer_dir: None,
    };

    let mock_dgvoodoo_dir = temp.join("mock_dgvoodoo");
    fs::create_dir_all(&mock_dgvoodoo_dir).unwrap();
    let mock_d3d9_x86 = mock_dgvoodoo_dir.join("D3D9_x86.dll");
    let mock_d3d9_x64 = mock_dgvoodoo_dir.join("D3D9_x64.dll");
    let mock_d3d8_x86 = mock_dgvoodoo_dir.join("D3D8_x86.dll");
    let mock_dg_conf = mock_dgvoodoo_dir.join("dgVoodoo.conf");
    fs::write(&mock_d3d9_x86, b"MOCK_DGVOODOO_D3D9_X86").unwrap();
    fs::write(&mock_d3d9_x64, b"MOCK_DGVOODOO_D3D9_X64").unwrap();
    fs::write(&mock_d3d8_x86, b"MOCK_DGVOODOO_D3D8_X86").unwrap();
    fs::write(&mock_dg_conf, b"[DirectX]\ndgVoodooWatermark = true\n[General]\nDisableAndPassThru = true\n").unwrap();

    let mock_dgvoodoo = dlss_studio::core::downloader::DgVoodooComponents {
        d3d9_x86: mock_d3d9_x86,
        d3d9_x64: mock_d3d9_x64,
        d3d8_x86: Some(mock_d3d8_x86),
        conf: mock_dg_conf,
    };

    let dlss_dll = temp.join("nvngx_dlss.dll");
    fs::write(&dlss_dll, b"MOCK_DLSS_PE_BYTES").unwrap();

    PayloadBundle {
        optiscaler_dll,
        optiscaler_ini,
        optiscaler_dir: Some(opti_dir.join("OptiScaler")),
        nvngx_dlss_dll: Some(dlss_dll),
        nvngx_dlssnr_dll: Some(nr_dll),
        nvngx_snippet_dll: Some(snippet),
        rtxmfg_dll: Some(rtxmfg),
        reshade64_dll: Some(reshade),
        reshade32_dll: Some(reshade32),
        renodx_dlss5_addon: Some(renodx),
        dlss5_d3d12_fix_addon: Some(fix),
        renodx_mfgunlock_addon: Some(mfgunlock),
        feeder_components: Some(mock_feeder),
        streamline_dir: Some(streamline_dir),
        dgvoodoo: Some(mock_dgvoodoo),
    }
}

#[test]
fn test_set_ini() {
    let base = "[DlssNr]\nRunBeforeSR=false\nPasses=1\n";
    let updated = set_ini(base, "DlssNr", "RunBeforeSR", "true");
    assert!(updated.contains("RunBeforeSR=true"));
    let updated2 = set_ini(&updated, "DlssNr", "Passes", "3");
    assert!(updated2.contains("Passes=3"));
}

#[test]
fn test_configure_dgvoodoo_conf_sets_display_duration_and_preserves_sections() {
    let sample = r#"[General]
OutputAPI = bestavailable
DisableAndPassThru=false

[GeneralExt]
WatermarkDisplayDuration = 0

[Glide]
3DfxWatermark = false
3DfxSplashScreen = true

[DirectX]
DisableAndPassThru = false
dgVoodooWatermark = false
"#;
    let configured = configure_dgvoodoo_conf(sample);
    assert!(configured.contains("WatermarkDisplayDuration = 3"), "Must set WatermarkDisplayDuration to 3");
    assert!(configured.contains("dgVoodooWatermark = true"), "Must enable dgVoodooWatermark for temporary 3s confirmation");
    assert!(configured.contains("3DfxWatermark = true"), "Must enable 3DfxWatermark for temporary 3s confirmation");
    assert!(configured.contains("3DfxSplashScreen = false"), "Must disable 3DfxSplashScreen intro animation");
    assert!(configured.contains("VRAM = 2048") || configured.contains("VRAM=2048"), "Must configure VRAM to 2048MB for high resolutions");
    assert!(!configured.contains("[General]\r\nOutputAPI = bestavailable\r\nDisableAndPassThru=false"), "Must strip invalid DisableAndPassThru from [General]");
}

#[test]
fn test_find_overlay_addon_payload() {
    let p = find_overlay_addon_payload();
    assert!(p.is_some(), "In-game overlay addon payload must always be discoverable or materialized");
    let path = p.unwrap();
    assert!(path.exists(), "Materialized overlay addon must exist on disk");

    let metadata = fs::metadata(&path).expect("metadata must be readable");
    assert!(
        metadata.len() >= 50_000,
        "Overlay addon must be valid 64-bit payload, found size: {}",
        metadata.len()
    );
}

#[test]
fn test_find_payloads() {
    if find_optiscaler_payload().is_none() || find_standalone_mfg_payload().is_none() {
        println!("OptiScaler or Standalone MFG payload not installed on this runner; skipping payload discovery test.");
        return;
    }
    assert!(find_optiscaler_payload().is_some(), "OptiScaler payload should be found");
    assert!(find_reshade64_payload().is_some(), "ReShade64 payload should be found");
    assert!(find_mfg_addon_payload().is_some(), "MFG addon payload should be found");
    assert!(find_dlssnr_payload().is_some(), "DLSS-NR payload should be found");
    assert!(find_standalone_mfg_payload().is_some(), "Standalone RTXMFG payload should be found");
}

#[test]
fn test_deploy_and_restore() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if find_optiscaler_payload().is_none() || find_standalone_mfg_payload().is_none() {
        println!("OptiScaler or Standalone MFG payload not installed on this runner; skipping deploy and restore test.");
        return;
    }
    let temp = TempDir::new("deploy_restore");
    let content_dir = temp.join("Content");
    fs::create_dir_all(&content_dir).unwrap();

    let orig_dxgi = content_dir.join("dxgi.dll");
    fs::write(&orig_dxgi, b"ORIGINAL_DXGI").unwrap();

    let exe_path = content_dir.join("Resonance.exe");
    fs::write(&exe_path, b"DUMMY_EXE").unwrap();

    let opts = DeployOptions {
        game_name: Some("Test Game".to_string()),
        game_dir: content_dir.clone(),
        exe_path: exe_path.clone(),
        api: "dxgi".to_string(),
        pre_sr: true,
        passes: 2,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_optiscaler(&opts).expect("deploy should succeed");
    assert!(res.success);
    assert_eq!(res.replaced, 1);
    assert!(res.added >= 2);

    // Verify deployed files
    assert!(orig_dxgi.exists());
    assert_ne!(fs::read(&orig_dxgi).unwrap(), b"ORIGINAL_DXGI");

    let opti_ini = fs::read_to_string(content_dir.join("OptiScaler.ini")).unwrap();
    assert!(opti_ini.contains("RunBeforeSR=true"));
    assert!(opti_ini.contains("Passes=2"));
    assert!(opti_ini.contains("LoadReshade=false"));
    assert!(opti_ini.contains("TargetProcessName=Resonance.exe"));

    // Standalone MFG as version.dll
    assert!(content_dir.join("version.dll").exists());
    // Pure OptiScaler - zero ReShade files
    assert!(!content_dir.join("ReShade64.dll").exists());
    assert!(!content_dir.join("renodx-mfgunlock.addon64").exists());
    assert!(content_dir.join("_DLSS5_Backup").join("manifest.json").exists());

    // Now test restore
    let rest = dlss_studio::core::journal::restore_game(&content_dir).expect("restore should succeed");
    assert!(rest);

    // Verify clean state
    assert_eq!(fs::read(&orig_dxgi).unwrap(), b"ORIGINAL_DXGI");
    assert!(!content_dir.join("version.dll").exists());
    assert!(!content_dir.join("ReShade64.dll").exists());
    assert!(!content_dir.join("renodx-mfgunlock.addon64").exists());
    assert!(!content_dir.join("_DLSS5_Backup").join("manifest.json").exists());
}

#[test]
fn test_addon_state_helpers() {
    use dlss_studio::core::state::*;
    let mut state = AppState::default();
    assert!(is_addon_active(&state, "builtin:renodx"));
    assert!(is_addon_active(&state, "builtin:mfgunlock"));
    assert!(is_addon_active(&state, "builtin:feeder"));
    assert!(!is_addon_active(&state, "builtin:overlay"));

    // Mandatory base add-ons cannot be deactivated
    toggle_addon_in_state(&mut state, "builtin:renodx", false);
    assert!(is_addon_active(&state, "builtin:renodx"), "Base add-ons are mandatory");

    let custom = AddonFileEntry {
        path: "C:\\mods\\my_addon.addon64".to_string(),
        name: Some("My Custom Addon".to_string()),
        tag: Some("HDR".to_string()),
        description: Some("Custom HDR grading".to_string()),
    };
    add_custom_addon(&mut state, custom);
    assert_eq!(state.addon_files.len(), 1);
    assert!(is_addon_active(&state, "C:\\mods\\my_addon.addon64"));

    toggle_addon_in_state(&mut state, "C:\\mods\\my_addon.addon64", false);
    assert!(!is_addon_active(&state, "C:\\mods\\my_addon.addon64"));
    toggle_addon_in_state(&mut state, "C:\\mods\\my_addon.addon64", true);
    assert!(is_addon_active(&state, "C:\\mods\\my_addon.addon64"));

    remove_custom_addon(&mut state, "C:\\mods\\my_addon.addon64");
    assert_eq!(state.addon_files.len(), 0);
    assert!(!is_addon_active(&state, "C:\\mods\\my_addon.addon64"));
}

#[test]
fn test_find_renodx_payload() {
    if find_renodx_payload().is_none() {
        println!("RenoDX payload not installed on this runner; skipping test.");
        return;
    }
    assert!(find_renodx_payload().is_some(), "RenoDX v4.7 payload should be found");
}

#[test]
fn test_deploy_respects_addon_toggles() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if find_optiscaler_payload().is_none() || find_renodx_payload().is_none() {
        println!("OptiScaler or RenoDX payload not installed on this runner; skipping addon toggle test.");
        return;
    }
    let temp = TempDir::new("addon_toggle");
    let content_dir = temp.join("Content");
    fs::create_dir_all(&content_dir).unwrap();

    let orig_dxgi = content_dir.join("dxgi.dll");
    fs::write(&orig_dxgi, b"ORIGINAL_DXGI").unwrap();

    let exe_path = content_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_EXE").unwrap();

    let custom_addon_file = temp.join("test_custom.addon64");
    fs::write(&custom_addon_file, b"CUSTOM_ADDON_PAYLOAD").unwrap();

    let opts = DeployOptions {
        game_name: Some("Addon Test Game".to_string()),
        game_dir: content_dir.clone(),
        exe_path: exe_path.clone(),
        api: "dxgi".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let mut state = dlss_studio::core::state::load_state();
    let custom_entry = dlss_studio::core::state::AddonFileEntry {
        path: custom_addon_file.to_string_lossy().to_string(),
        name: Some("Test Custom".to_string()),
        tag: Some("Test".to_string()),
        description: None,
    };
    dlss_studio::core::state::add_custom_addon(&mut state, custom_entry);
    let _ = dlss_studio::core::state::save_state(&state);

    let res = deploy_native_dlss5(&opts).expect("deploy should succeed");
    assert!(res.success);
    assert!(content_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 should be deployed as mandatory base");
    assert!(content_dir.join("test_custom.addon64").exists(), "custom addon should be deployed when active");

    let rest_res = dlss_studio::core::journal::restore_game(&content_dir).expect("restore should succeed");
    assert!(rest_res);
    assert!(!content_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 should be deleted on restore");
    assert!(!content_dir.join("test_custom.addon64").exists(), "custom addon should be deleted on restore");

    dlss_studio::core::state::toggle_addon_in_state(&mut state, &custom_addon_file.to_string_lossy(), false);
    let _ = dlss_studio::core::state::save_state(&state);

    let res2 = deploy_native_dlss5(&opts).expect("deploy should succeed");
    assert!(res2.success);
    assert!(content_dir.join("renodx-dlss5.addon64").exists(), "mandatory base addon is still deployed");
    assert!(!content_dir.join("test_custom.addon64").exists(), "custom addon should NOT be deployed when deactivated");

    dlss_studio::core::state::remove_custom_addon(&mut state, &custom_addon_file.to_string_lossy());
    let _ = dlss_studio::core::state::save_state(&state);
}

#[test]
fn test_native_dlss5_nested_deploy_and_clean_lifecycle() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if find_optiscaler_payload().is_none() || find_renodx_payload().is_none() {
        println!("OptiScaler or RenoDX payload not installed on this runner; skipping test.");
        return;
    }
    let orig_state = dlss_studio::core::state::load_state();
    let mut state = orig_state.clone();
    if !state.addons.iter().any(|a| a == "builtin:mfgunlock") {
        state.addons.push("builtin:mfgunlock".to_string());
    }
    if !state.addons.iter().any(|a| a == "builtin:renodx") {
        state.addons.push("builtin:renodx".to_string());
    }
    let _ = dlss_studio::core::state::save_state(&state);

    let temp = TempDir::new("nested_lifecycle");
    let bin_dir = temp.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("CyberGame.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();

    let orig_dlss = bin_dir.join("nvngx_dlss.dll");
    fs::write(&orig_dlss, b"ORIGINAL_DLSS_BINARY").unwrap();
    fs::write(bin_dir.join("D3D12Core.dll"), b"core").unwrap();

    let opts = DeployOptions {
        game_name: Some("CyberGame".to_string()),
        game_dir: temp.path().to_path_buf(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_native_dlss5(&opts).expect("deploy_native_dlss5 should succeed");
    assert!(res.success);

    assert!(bin_dir.join("dxgi.dll").exists(), "dxgi.dll hook should be deployed in nested bin/x64");
    assert!(bin_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 should be in bin/x64");
    assert!(bin_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 should be in bin/x64");
    assert!(bin_dir.join("ReShade.ini").exists(), "ReShade.ini should be in bin/x64");

    let scanned = dlss_studio::core::scan::scan_game_directory(temp.path()).expect("game should be recognized");
    assert!(scanned.reshade_installed, "Reshade must be detected after deploy");
    assert!(scanned.addon_installed, "Add-on must be detected after deploy");
    assert_eq!(scanned.installed_route, Some("native".to_string()));

    let removed = dlss_studio::core::journal::clean_untracked_mods_with_exe(temp.path(), Some(&exe_path)).expect("clean should succeed");
    assert!(removed.iter().any(|r| r.contains("dxgi.dll")), "Cleaned files must include dxgi.dll: {:?}", removed);
    assert!(removed.iter().any(|r| r.contains("renodx-dlss5.addon64")), "Cleaned files must include renodx-dlss5.addon64: {:?}", removed);
    assert!(removed.iter().any(|r| r.contains("renodx-mfgunlock.addon64")), "Cleaned files must include renodx-mfgunlock.addon64: {:?}", removed);

    assert!(!bin_dir.join("dxgi.dll").exists(), "dxgi.dll must be purged from bin/x64");
    assert!(!bin_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be purged from bin/x64");
    assert!(!bin_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must be purged from bin/x64");
    assert!(!bin_dir.join("ReShade.ini").exists(), "ReShade.ini must be purged from bin/x64");
    assert!(bin_dir.join("CyberGame.exe").exists(), "Original game executable must not be deleted");
    assert_eq!(fs::read(&orig_dlss).unwrap(), b"ORIGINAL_DLSS_BINARY", "Original DLSS file must remain untouched");

    let after_clean = dlss_studio::core::scan::scan_game_directory(temp.path()).expect("game should be recognized after clean");
    assert!(!after_clean.reshade_installed, "ReShade must be false after clean");
    assert!(!after_clean.addon_installed, "Add-on must be false after clean");
    assert_eq!(after_clean.installed_route, None, "Installed route must be None after clean");
    let _ = dlss_studio::core::state::save_state(&orig_state);
}

#[test]
fn test_prevent_reshade_hook_corrupted_into_backup() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if find_optiscaler_payload().is_none() || find_renodx_payload().is_none() {
        println!("OptiScaler or RenoDX payload not installed on this runner; skipping test.");
        return;
    }
    let temp = TempDir::new("dirty_backup");
    let bin_dir = temp.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("Game.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();
    fs::write(bin_dir.join("D3D12Core.dll"), b"core").unwrap();

    let pre_existing_dxgi = bin_dir.join("dxgi.dll");
    let mut reshade_bytes = vec![0u8; 50000];
    reshade_bytes[20000..20007].copy_from_slice(b"ReShade");
    fs::write(&pre_existing_dxgi, &reshade_bytes).unwrap();

    assert!(is_known_mod_file(&pre_existing_dxgi), "is_known_mod_file must recognize pre-existing ReShade dxgi.dll");

    let opts = DeployOptions {
        game_name: Some("Game".to_string()),
        game_dir: temp.path().to_path_buf(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_native_dlss5(&opts).expect("deploy should succeed");
    assert!(res.success);

    let backup_dxgi = temp.join("_DLSS5_Backup").join("bin").join("x64").join("dxgi.dll");
    assert!(!backup_dxgi.exists(), "ReShade hook must never be copied to _DLSS5_Backup as an original file");

    let manifest = dlss_studio::core::journal::read_manifest(temp.path()).expect("manifest should exist");
    assert!(!manifest.replaced.iter().any(|r| r.rel.contains("dxgi.dll")), "manifest.replaced must not contain dxgi.dll");

    let restored = dlss_studio::core::journal::restore_game(temp.path()).expect("restore should succeed");
    assert!(restored);

    assert!(!bin_dir.join("dxgi.dll").exists(), "Restoring must not leave or restore ReShade hook");

    let scanned = dlss_studio::core::scan::scan_game_directory(temp.path()).expect("scan should succeed");
    assert!(!scanned.reshade_installed, "ReShade must be false after restore");
}

#[test]
fn test_overlay_addon_strictly_not_deployed_while_shelved() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if find_optiscaler_payload().is_none() || find_renodx_payload().is_none() {
        println!("OptiScaler or RenoDX payload not installed on this runner; skipping test.");
        return;
    }

    let temp = TempDir::new("overlay_shelved");
    let bin_dir = temp.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("CyberGame.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();
    fs::write(bin_dir.join("D3D12Core.dll"), b"core").unwrap();

    let orig_state = dlss_studio::core::state::load_state();

    let opts = DeployOptions {
        game_name: Some("CyberGame".to_string()),
        game_dir: temp.path().to_path_buf(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_native_dlss5(&opts).expect("deploy_native_dlss5 should succeed");
    assert!(res.success);

    let deployed_overlay = bin_dir.join("dlss5-lab-overlay.addon64");
    assert!(!deployed_overlay.exists(), "dlss5-lab-overlay.addon64 must NEVER be deployed while shelved");

    let manifest = dlss_studio::core::journal::read_manifest(temp.path()).expect("manifest should exist");
    assert!(!manifest.added.iter().any(|a| a.contains("dlss5-lab-overlay.addon64")), "manifest.added must NOT contain dlss5-lab-overlay.addon64");

    let _ = dlss_studio::core::journal::clean_untracked_mods_with_exe(temp.path(), Some(&exe_path));
    let _ = dlss_studio::core::state::save_state(&orig_state);
}

#[test]
fn test_overlay_addon_clean_and_restore_lifecycle() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let temp = TempDir::new("overlay_lifecycle");
    let bin_dir = temp.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("CyberGame.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();
    fs::write(bin_dir.join("D3D12Core.dll"), b"core").unwrap();

    fs::write(bin_dir.join("dlss5-lab-overlay.addon64"), b"OLD_OVERLAY_BYTES").unwrap();
    assert!(bin_dir.join("dlss5-lab-overlay.addon64").exists(), "Simulated overlay addon must exist before clean");

    let removed = dlss_studio::core::journal::clean_untracked_mods_with_exe(temp.path(), Some(&exe_path)).expect("clean should succeed");
    assert!(removed.iter().any(|r| r.contains("dlss5-lab-overlay.addon64")), "Removed files must include dlss5-lab-overlay.addon64: {:?}", removed);

    assert!(!bin_dir.join("dlss5-lab-overlay.addon64").exists(), "dlss5-lab-overlay.addon64 must be purged from disk");

    let scanned = dlss_studio::core::scan::scan_game_directory(temp.path()).expect("scan should succeed");
    assert!(!scanned.addon_installed, "addon_installed must be false after clean");
}

#[test]
fn test_deploy_preserves_reshade_defaults() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if find_optiscaler_payload().is_none() || find_renodx_payload().is_none() {
        println!("OptiScaler or RenoDX payload not installed on this runner; skipping test.");
        return;
    }
    let temp = TempDir::new("font_test");
    let bin_dir = temp.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("FontGame.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();
    fs::write(bin_dir.join("D3D12Core.dll"), b"core").unwrap();

    let opts = DeployOptions {
        game_name: Some("FontGame".to_string()),
        game_dir: temp.path().to_path_buf(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_native_dlss5(&opts).expect("deploy should succeed");
    assert!(res.success);

    let reshade_ini_path = bin_dir.join("ReShade.ini");
    assert!(reshade_ini_path.exists(), "ReShade.ini must be deployed");

    let ini_content = fs::read_to_string(&reshade_ini_path).unwrap();
    let font = get_ini(&ini_content, "STYLE", "Font");
    assert_eq!(font, None);

    let _ = dlss_studio::core::journal::clean_untracked_mods_with_exe(temp.path(), Some(&exe_path));
}

#[test]
fn test_optiscaler_route_deploys_expected_files_and_strictly_excludes_reshade() {
    let temp = TempDir::new("pure_opti");
    let game_dir = temp.join("GameDir");
    let bin_dir = game_dir.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Pure Opti Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 2,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy_optiscaler_with_bundle must succeed");
    assert!(res.success);

    // 1. Positive assertions: Pure OptiScaler and Standalone RTXMFG must exist
    assert!(bin_dir.join("dxgi.dll").exists(), "OptiScaler dxgi.dll must be deployed");
    assert!(bin_dir.join("version.dll").exists(), "Standalone RTXMFG version.dll must be deployed");
    assert!(bin_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must be deployed");
    assert!(bin_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed");
    assert!(bin_dir.join("nvngx.dll_dlssnr.dll").exists(), "nvngx.dll_dlssnr.dll must be deployed");

    // 2. Strict Negative assertions: ZERO ReShade or add-on files allowed
    assert!(!bin_dir.join("ReShade64.dll").exists(), "ReShade64.dll must NEVER be deployed in OptiScaler route");
    assert!(!bin_dir.join("ReShade.ini").exists(), "ReShade.ini must NEVER be deployed in OptiScaler route");
    assert!(!bin_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must NEVER be deployed in OptiScaler route");
    assert!(!bin_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must NEVER be deployed in OptiScaler route");
    assert!(!bin_dir.join("dlss5-lab-overlay.addon64").exists(), "overlay addon must NEVER be deployed in OptiScaler route");

    // 3. Ini inspection: LoadReshade must be false, External must be true, PreSR configured
    let ini_content = fs::read_to_string(bin_dir.join("OptiScaler.ini")).unwrap();
    assert_eq!(get_ini(&ini_content, "Plugins", "LoadReshade"), Some("false".to_string()));
    assert_eq!(get_ini(&ini_content, "FrameGen", "External"), Some("true".to_string()));
    assert_eq!(get_ini(&ini_content, "DlssNr", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(&ini_content, "DlssNr", "RunBeforeSR"), Some("true".to_string()));
    assert_eq!(get_ini(&ini_content, "DlssNr", "Passes"), Some("2".to_string()));
}

#[test]
fn test_optiscaler_route_without_mfg() {
    let temp = TempDir::new("opti_no_mfg");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Opti No MFG".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "dxgi".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    assert!(!game_dir.join("version.dll").exists(), "version.dll must not be deployed when mfg_unlock is false");
    let ini_content = fs::read_to_string(game_dir.join("OptiScaler.ini")).unwrap();
    assert_eq!(get_ini(&ini_content, "FrameGen", "External"), Some("false".to_string()));
}

#[test]
fn test_streamline_game_deploys_version_dll_with_external_framegen() {
    let temp = TempDir::new("streamline_mfg");
    let game_dir = temp.join("Cyberpunk2077");
    let bin_dir = game_dir.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("Cyberpunk2077.exe");
    fs::write(&exe_path, b"DUMMY_CYBERPUNK_EXE").unwrap();

    // Simulate native Streamline files present in Cyberpunk 2077
    fs::write(bin_dir.join("sl.interposer.dll"), b"MOCK_STREAMLINE_INTERPOSER").unwrap();
    fs::write(bin_dir.join("sl.common.dll"), b"MOCK_STREAMLINE_COMMON").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Cyberpunk 2077".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    // Positive assertions: OptiScaler and DLSS-NR files deployed
    assert!(bin_dir.join("dxgi.dll").exists(), "OptiScaler dxgi.dll must be deployed");
    assert!(bin_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed");
    assert!(bin_dir.join("nvngx.dll_dlssnr.dll").exists(), "nvngx.dll_dlssnr.dll must be deployed");
    assert!(bin_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must be deployed");

    assert!(
        bin_dir.join("version.dll").exists(),
        "Standalone RTXMFG version.dll must be deployed to provide 4x MFG unlock"
    );
    assert!(
        bin_dir.join("RTXMFG-Universal.json").exists(),
        "RTXMFG-Universal.json configuration must be deployed"
    );
    assert!(
        bin_dir.join("RTX40MFG-Universal.json").exists(),
        "RTX40MFG-Universal.json configuration must be deployed"
    );

    assert_eq!(fs::read(bin_dir.join("sl.interposer.dll")).unwrap(), b"MOCK_SL_INTERPOSER");
    assert_eq!(fs::read(bin_dir.join("sl.common.dll")).unwrap(), b"MOCK_SL_COMMON");
    assert_eq!(fs::read(bin_dir.join("sl.reflex.dll")).unwrap(), b"MOCK_SL_REFLEX");

    let ini_content = fs::read_to_string(bin_dir.join("OptiScaler.ini")).unwrap();
    assert_eq!(get_ini(&ini_content, "FrameGen", "External"), Some("true".to_string()));
    assert_eq!(get_ini(&ini_content, "DLSSG", "InterpolationCount"), Some("3".to_string()));
    assert_eq!(get_ini(&ini_content, "DLSSG", "OverrideInterpolationCount"), Some("true".to_string()));

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);

    assert_eq!(fs::read(bin_dir.join("sl.interposer.dll")).unwrap(), b"MOCK_STREAMLINE_INTERPOSER");
    assert_eq!(fs::read(bin_dir.join("sl.common.dll")).unwrap(), b"MOCK_STREAMLINE_COMMON");
    assert!(!bin_dir.join("sl.reflex.dll").exists(), "sl.reflex.dll must be wiped on restore");
    assert!(!bin_dir.join("version.dll").exists(), "version.dll must be wiped on restore");
    assert!(!bin_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must be wiped on restore");
}

#[test]
fn test_streamline_1x_game_preserves_original_files_when_deploying_optiscaler() {
    let temp = TempDir::new("sl1_preserve");
    let game_dir = temp.join("PlagueTaleRequiem");
    let bin_dir = game_dir.clone();
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("APT2_WinStore.x64.Submission.exe");
    fs::write(&exe_path, b"DUMMY_APT2_EXE").unwrap();

    let mut sl1_bytes = vec![0u8; 4096];
    sl1_bytes[100..121].copy_from_slice(b"slGetFeatureSettings\0");
    fs::write(bin_dir.join("sl.interposer.dll"), &sl1_bytes).unwrap();
    fs::write(bin_dir.join("sl.common.dll"), b"ORIGINAL_SL1_COMMON").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("A Plague Tale: Requiem".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 2,
        nr_style_enabled: true,
    };

    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    assert!(bin_dir.join("dxgi.dll").exists(), "OptiScaler dxgi.dll must be deployed");
    assert!(bin_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed");
    assert!(bin_dir.join("nvngx.dll_dlssnr.dll").exists(), "nvngx.dll_dlssnr.dll must be deployed");
    assert!(bin_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must be deployed");

    assert_eq!(
        fs::read(bin_dir.join("sl.interposer.dll")).unwrap(),
        sl1_bytes,
        "Streamline 1.x sl.interposer.dll must be strictly preserved to prevent export crashes"
    );
    assert_eq!(
        fs::read(bin_dir.join("sl.common.dll")).unwrap(),
        b"ORIGINAL_SL1_COMMON",
        "Streamline 1.x sl.common.dll must be strictly preserved"
    );
    assert!(!bin_dir.join("sl.reflex.dll").exists(), "sl.reflex.dll must NOT be deployed to Streamline 1.x titles");
}

#[test]
fn test_reshade_route_deploys_expected_files_and_strictly_excludes_optiscaler() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("pure_reshade");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("ReShade Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_native_dlss5_with_bundle(&opts, &payloads).expect("deploy_native_dlss5_with_bundle must succeed");
    assert!(res.success);

    assert!(game_dir.join("dxgi.dll").exists(), "ReShade dxgi.dll must be deployed");
    assert!(game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be deployed");
    assert!(game_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed for RenoDX DLSS 5 Neural Rendering");
    assert!(game_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must be deployed");
    assert!(game_dir.join("ReShade.ini").exists(), "ReShade.ini must be deployed");

    assert!(!game_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must NEVER be deployed in ReShade route");
    assert!(!game_dir.join("OptiScaler").exists(), "OptiScaler directory must NEVER be deployed in ReShade route");
    assert!(!game_dir.join("version.dll").exists(), "Standalone RTXMFG version.dll must NEVER be deployed in ReShade route");
    assert!(!game_dir.join("nvngx.dll_dlssnr.dll").exists(), "nvngx.dll_dlssnr.dll must NEVER be in ReShade route");
}

#[test]
fn test_reshade_route_mfg_disabled() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("pure_reshade_no_mfg");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("ReShade No MFG".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_native_dlss5_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    assert!(!game_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must NOT be deployed when mfg_unlock is false");
}

#[test]
fn test_feeder_route_deploys_only_feeder_and_strictly_excludes_mfg_and_presr() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_pure");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Feeder Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "dxgi".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle must succeed");
    assert!(res.success);

    assert!(game_dir.join("dxgi.dll").exists(), "ReShade dxgi.dll must be deployed");
    assert!(game_dir.join("ReShade.ini").exists(), "ReShade.ini must be deployed");
    assert!(game_dir.join("dlss5-feed.addon64").exists(), "dlss5-feed.addon64 must be deployed");
    assert!(game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be deployed for neural rendering");
    assert!(game_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed for neural rendering");
    assert!(game_dir.join("nvngx_dlss.dll").exists(), "nvngx_dlss.dll must be deployed for Streamline Feeder Super Sampling");
    assert!(game_dir.join("reshade-shaders").join("Shaders").join("DrawText.fxh").exists(), "DrawText.fxh must be deployed");
    assert!(game_dir.join("reshade-shaders").join("Textures").join("FontAtlas.png").exists(), "FontAtlas.png must be deployed");

    assert!(!game_dir.join("version.dll").exists(), "version.dll must NOT be in Feeder route");
    assert!(!game_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must NOT be in Feeder route when mfg_unlock is false");
    assert!(!game_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must NOT be in Feeder route");
    assert!(!game_dir.join("OptiScaler").exists(), "OptiScaler dir must NOT be in Feeder route");
}

#[test]
fn test_feeder_route_deploys_mfg_and_dlssnr_when_mfg_unlock_enabled() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_mfg");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Feeder MFG Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "dxgi".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle must succeed");
    assert!(res.success);

    assert!(game_dir.join("dxgi.dll").exists(), "ReShade dxgi.dll must be deployed");
    assert!(game_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must be deployed on Feeder with MFG");
    assert!(game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be deployed on Feeder with MFG");
    assert!(game_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed on Feeder with MFG");
    assert!(game_dir.join("ReShade.ini").exists(), "ReShade.ini must be deployed");

    let ini_content = fs::read_to_string(game_dir.join("ReShade.ini")).unwrap();
    assert!(ini_content.contains("[RenoDX.MFGUnlock]"), "[RenoDX.MFGUnlock] section must be written");
    assert!(ini_content.contains("ForceMultiplier=4"), "ForceMultiplier=4 must be written for 4x frame generation");
    assert!(ini_content.contains("EnableHooks=1"), "ReShade.ini in Feeder mode specifies EnableHooks=1 for swapchain and direct presentation interception");
    assert!(ini_content.contains("NeuralUplift=1"), "ReShade.ini in Feeder mode specifies NeuralUplift=1 for Neural Rendering");

    let preset_content = fs::read_to_string(game_dir.join("ReShadePreset.ini")).unwrap();
    assert!(preset_content.starts_with("Techniques=vort_MotionEffects@vort_Motion.fx,DLSS5_Feed@DLSS5_Feed.fx"), "Techniques must be at root of ReShadePreset.ini");
    assert!(!preset_content.starts_with("[ReShadePreset.ini]"), "ReShadePreset.ini must NOT contain a section header at line 1");
}

#[test]
fn test_feeder_route_dx11_configures_preset_root_and_enables_mfg() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_dx11");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game_DX11.exe");
    fs::write(&exe_path, b"DUMMY_DX11_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("DirectX 11 Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 11".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle must succeed for DX11");
    assert!(res.success);

    assert!(game_dir.join("dxgi.dll").exists(), "ReShade dxgi.dll must be deployed for DX11");
    assert!(game_dir.join("dlss5-feed.addon64").exists(), "dlss5-feed.addon64 must be deployed for DX11");
    assert!(game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be deployed for DX11");
    assert!(game_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed for DX11");
    assert!(game_dir.join("nvngx_dlss.dll").exists(), "nvngx_dlss.dll must be deployed for DX11");
    assert!(game_dir.join("ReShade.ini").exists(), "ReShade.ini must be deployed for DX11");

    assert!(!game_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must NOT be deployed on DX11 games");

    let ini_content = fs::read_to_string(game_dir.join("ReShade.ini")).unwrap();
    assert!(ini_content.contains("EnableHooks=1"), "ReShade.ini specifies EnableHooks=1 for swapchain and direct presentation interception");
    assert!(ini_content.contains("NeuralUplift=1"), "ReShade.ini in DX11 Feeder mode specifies NeuralUplift=1 for Neural Rendering");
    assert!(!ini_content.contains("[RenoDX.MFGUnlock]"), "[RenoDX.MFGUnlock] section must NOT be written for DX11 titles without native DLSS-G");
}

#[test]
fn test_feeder_route_vulkan_uses_dxgi_and_cleans_stale_winmm() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_vulkan");
    let game_dir = temp.join("GameDir");
    let bin_dir = game_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let stale_winmm = bin_dir.join("winmm.dll");
    fs::write(&stale_winmm, b"MOCK_RESHADE_PE_BYTES").unwrap();
    assert!(stale_winmm.exists());

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Vulkan Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "Vulkan".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle must succeed");
    assert!(res.success);

    assert!(bin_dir.join("dxgi.dll").exists(), "dxgi.dll must be deployed for Vulkan game");
    assert!(bin_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must be deployed for Vulkan game with MFG unlock");
    let ini_content = fs::read_to_string(bin_dir.join("ReShade.ini")).unwrap();
    assert!(ini_content.contains("[RenoDX.MFGUnlock]"), "[RenoDX.MFGUnlock] must be written for Vulkan game with MFG unlock");

    assert!(!bin_dir.join("winmm.dll").exists(), "Obsolete winmm.dll proxy must be deleted so timeGetTime is not intercepted");
}

#[test]
fn test_route_clean_and_rollback() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("rollback");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let orig_dxgi = game_dir.join("dxgi.dll");
    fs::write(&orig_dxgi, b"GENUINE_GAME_DXGI").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Rollback Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "dxgi".to_string(),
        pre_sr: true,
        passes: 2,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_optiscaler_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);
    assert_eq!(fs::read(&orig_dxgi).unwrap(), b"MOCK_OPTISCALER_PE_BYTES");

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);

    assert_eq!(fs::read(&orig_dxgi).unwrap(), b"GENUINE_GAME_DXGI");
    assert!(!game_dir.join("version.dll").exists(), "version.dll must be wiped on restore");
    assert!(!game_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must be wiped on restore");
}

#[test]
fn test_switching_routes_cleans_previous_artifacts() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("switch_routes");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opti_opts = DeployOptions {
        game_name: Some("Switch Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 2,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };
    deploy_optiscaler_with_bundle(&opti_opts, &payloads).unwrap();
    assert!(game_dir.join("OptiScaler.ini").exists());
    assert!(game_dir.join("version.dll").exists());

    let removed = dlss_studio::core::journal::clean_untracked_mods_with_exe(&game_dir, Some(&exe_path)).unwrap();
    assert!(removed.iter().any(|r| r.contains("OptiScaler.ini")));

    let reshade_opts = DeployOptions {
        game_name: Some("Switch Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };
    deploy_native_dlss5_with_bundle(&reshade_opts, &payloads).unwrap();

    assert!(game_dir.join("renodx-dlss5.addon64").exists());
    assert!(game_dir.join("ReShade.ini").exists());
    assert!(!game_dir.join("OptiScaler.ini").exists());
    assert!(!game_dir.join("version.dll").exists());
}

#[test]
fn test_deploy_fails_cleanly_on_missing_payload() {
    let temp = TempDir::new("fail_cleanly");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let mut payloads = create_mock_payload_bundle(&temp.join("payloads"));
    payloads.optiscaler_dll = temp.join("non_existent_optiscaler.dll");

    let opts = DeployOptions {
        game_name: Some("Fail Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "dxgi".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_optiscaler_with_bundle(&opts, &payloads);
    assert!(res.is_err(), "Must return Err when critical payload is missing");
}

#[test]
fn test_ini_parser_handles_malformed_input() {
    let malformed = "just a random line\nno_equal_sign\n[DlssNr]\n=empty_key\nEnabled=true\n\n[SectionWithoutValue]\nKey=";
    assert_eq!(get_ini(malformed, "DlssNr", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(malformed, "SectionWithoutValue", "Key"), Some("".to_string()));
    assert_eq!(get_ini(malformed, "NonExistent", "Key"), None);

    let updated = set_ini(malformed, "DlssNr", "Passes", "3");
    assert_eq!(get_ini(&updated, "DlssNr", "Passes"), Some("3".to_string()));

    let added_section = set_ini("", "NewSection", "NewKey", "NewVal");
    assert_eq!(get_ini(&added_section, "NewSection", "NewKey"), Some("NewVal".to_string()));
}

#[test]
fn test_generate_optiscaler_ini_all_combinations() {
    let base = "[DlssNr]\nEnabled=false\n\n[Plugins]\nLoadReshade=true\n";

    let res_a = generate_optiscaler_ini(base, true, 3, true, Some("CyberGame.exe"), 2, Some("DirectX 12"), 4);
    assert_eq!(get_ini(&res_a, "DlssNr", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "DlssNr", "RunBeforeSR"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "DlssNr", "Passes"), Some("3".to_string()));
    assert_eq!(get_ini(&res_a, "DlssNr", "ApplyAfterRR"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "DlssNr", "Style"), Some("2".to_string()));
    assert_eq!(get_ini(&res_a, "Plugins", "LoadReshade"), Some("false".to_string()));
    assert_eq!(get_ini(&res_a, "FrameGen", "External"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "FrameGen", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "Dx11withDx12", "BuiltinMfgUnlock"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "MfgUnlock", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "MfgUnlock", "Multiplier"), Some("4".to_string()));
    assert_eq!(get_ini(&res_a, "DLSSG", "Multiplier"), Some("4".to_string()));
    assert_eq!(get_ini(&res_a, "DLSSG", "AdaMfgUnlock"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "DLSSG", "InterpolationCount"), Some("3".to_string()));
    assert_eq!(get_ini(&res_a, "DLSSG", "OverrideInterpolationCount"), Some("true".to_string()));
    assert_eq!(get_ini(&res_a, "Menu", "ShortcutKey"), Some("0x2D".to_string()));
    assert_eq!(get_ini(&res_a, "Init", "TargetProcessName"), Some("CyberGame.exe".to_string()));
    assert_eq!(get_ini(&res_a, "Upscalers", "Dx12Upscaler"), Some("dlss".to_string()));
    assert_eq!(get_ini(&res_a, "Spoofing", "StreamlineSpoofing"), Some("false".to_string()));
    assert_eq!(get_ini(&res_a, "Spoofing", "Dxgi"), Some("false".to_string()));

    let res_b = generate_optiscaler_ini(base, false, 1, false, None, 0, None, 4);
    assert_eq!(get_ini(&res_b, "DlssNr", "Enabled"), Some("false".to_string()));
    assert_eq!(get_ini(&res_b, "DlssNr", "RunBeforeSR"), Some("false".to_string()));
    assert_eq!(get_ini(&res_b, "DlssNr", "Passes"), Some("1".to_string()));
    assert_eq!(get_ini(&res_b, "Plugins", "LoadReshade"), Some("false".to_string()));
    assert_eq!(get_ini(&res_b, "FrameGen", "External"), Some("false".to_string()));
    assert_eq!(get_ini(&res_b, "FrameGen", "Enabled"), Some("false".to_string()));
    assert_eq!(get_ini(&res_b, "Dx11withDx12", "BuiltinMfgUnlock"), Some("false".to_string()));
    assert_eq!(get_ini(&res_b, "MfgUnlock", "Enabled"), Some("false".to_string()));
    assert_eq!(get_ini(&res_b, "MfgUnlock", "Multiplier"), Some("auto".to_string()));
    assert_eq!(get_ini(&res_b, "DLSSG", "Multiplier"), Some("auto".to_string()));
    assert_eq!(get_ini(&res_b, "DLSSG", "AdaMfgUnlock"), Some("false".to_string()));

    // DX11 games with MFG enabled automatically configure D3D11 to D3D12 bridge, OptiFG upscaler inputs, and delayed init
    let res_dx11 = generate_optiscaler_ini(base, true, 1, true, Some("bg3_dx11.exe"), 0, Some("DirectX 11"), 4);
    assert_eq!(get_ini(&res_dx11, "Upscalers", "Dx11Upscaler"), Some("dlss_12".to_string()));
    assert_eq!(get_ini(&res_dx11, "FrameGen", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(&res_dx11, "FrameGen", "FGInput"), Some("upscaler".to_string()));
    assert_eq!(get_ini(&res_dx11, "FrameGen", "FGOutput"), Some("dlssg".to_string()));
    assert_eq!(get_ini(&res_dx11, "FrameGen", "FGNvngxReplacement"), Some("None".to_string()));
    assert_eq!(get_ini(&res_dx11, "OptiFG", "HUDFix"), Some("true".to_string()));
    assert_eq!(get_ini(&res_dx11, "Dx11withDx12", "BuiltinMfgUnlock"), Some("true".to_string()));
    assert_eq!(get_ini(&res_dx11, "Dx11withDx12", "UseDelayedInit"), Some("true".to_string()));
    assert_eq!(get_ini(&res_dx11, "MfgUnlock", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(&res_dx11, "MfgUnlock", "Multiplier"), Some("4".to_string()));
    assert_eq!(get_ini(&res_dx11, "DLSSG", "Multiplier"), Some("4".to_string()));
    assert_eq!(get_ini(&res_dx11, "DLSSG", "AdaMfgUnlock"), Some("true".to_string()));
    assert_eq!(get_ini(&res_dx11, "DLSSG", "InterpolationCount"), Some("3".to_string()));
    assert_eq!(get_ini(&res_dx11, "Spoofing", "StreamlineSpoofing"), Some("false".to_string()));

    // Vulkan games automatically configure native DLSS
    let res_vk = generate_optiscaler_ini(base, true, 1, true, Some("bg3.exe"), 0, Some("Vulkan"), 4);
    assert_eq!(get_ini(&res_vk, "Upscalers", "VulkanUpscaler"), Some("dlss".to_string()));
    assert_eq!(get_ini(&res_vk, "Spoofing", "StreamlineSpoofing"), Some("false".to_string()));
}

#[test]
fn test_dynamic_optiscaler_payload_scoring_prioritizes_rtx40_mfg_and_latest_version() {
    let score_rtx40_085 = score_optiscaler_dir("OptiScaler-NR-v0.8.5-rtx40-mfg");
    let score_rtx40_084 = score_optiscaler_dir("OptiScaler-NR-v0.8.4-rtx40-mfg");
    let score_base_085 = score_optiscaler_dir("OptiScaler-0.8.5-dlssnr");
    let score_base_084 = score_optiscaler_dir("OptiScaler-0.8.4-dlssnr");

    // rtx40-mfg builds must score higher than base builds
    assert!(score_rtx40_085 > score_base_085, "rtx40-mfg builds must have priority over base builds");
    assert!(score_rtx40_084 > score_base_084, "rtx40-mfg builds must have priority over base builds");

    // newer version must score higher than older version within same tier
    assert!(score_rtx40_085 > score_rtx40_084, "v0.8.5 must score higher than v0.8.4");
    assert!(score_base_085 > score_base_084, "v0.8.5 must score higher than v0.8.4");
}

#[test]
fn test_payload_bundle_from_system_finds_streamline() {
    let bundle = match PayloadBundle::from_system() {
        Ok(b) => b,
        Err(_) => {
            println!("PayloadBundle not installed on this runner; skipping test.");
            return;
        }
    };
    if bundle.streamline_dir.is_none() || bundle.nvngx_snippet_dll.is_none() {
        println!("Streamline or snippet payload not installed on this runner; skipping test.");
        return;
    }
    let dir = bundle.streamline_dir.unwrap();
    assert!(dir.join("sl.interposer.dll").exists());
    assert!(dir.join("sl.common.dll").exists());
    assert!(dir.join("sl.reflex.dll").exists());
    assert!(dir.join("sl.dlss_g.dll").exists());
    assert!(dir.join("nvngx_dlssg.dll").exists());

    let snippet = bundle.nvngx_snippet_dll.unwrap();
    assert!(snippet.is_file(), "nvngx.dll_dlssnr.dll must be a valid file");
}

#[test]
fn test_feeder_preset_root_technique_formatting() {
    let fresh = configure_feeder_preset("");
    let first_line = fresh.lines().next().unwrap_or("");
    assert!(
        first_line.starts_with("Techniques=vort_MotionEffects@vort_Motion.fx,DLSS5_Feed@DLSS5_Feed.fx"),
        "First line of preset must be Techniques= and NOT a section header"
    );
    assert!(!fresh.starts_with('['), "Preset must not start with a section header");
    assert!(fresh.contains("TechniqueSorting=vort_MotionEffects@vort_Motion.fx,DLSS5_Feed@DLSS5_Feed.fx"));
    assert!(fresh.contains("[DLSS5_Feed.fx]"));
    assert!(fresh.contains("PreprocessorDefinitions=DLSS5_MV_PROVIDER=2"));

    let legacy = "[ReShadePreset.ini]\nTechniques=vort_MotionEffects@vort_Motion.fx\nTechniqueSorting=vort_MotionEffects@vort_Motion.fx\n";
    let fixed = configure_feeder_preset(legacy);
    assert!(!fixed.contains("[ReShadePreset.ini]"), "Erroneous section header must be stripped");
    assert!(fixed.starts_with("Techniques=vort_MotionEffects@vort_Motion.fx,DLSS5_Feed@DLSS5_Feed.fx"));

    let existing = "Techniques=CAS@CAS.fx,SMAA@SMAA.fx\nTechniqueSorting=CAS@CAS.fx,SMAA@SMAA.fx\n\n[CAS.fx]\nContrast=0.5\n";
    let merged = configure_feeder_preset(existing);
    assert!(merged.starts_with("Techniques=vort_MotionEffects@vort_Motion.fx,DLSS5_Feed@DLSS5_Feed.fx,CAS@CAS.fx,SMAA@SMAA.fx"));
    assert!(merged.contains("TechniqueSorting=vort_MotionEffects@vort_Motion.fx,DLSS5_Feed@DLSS5_Feed.fx,CAS@CAS.fx,SMAA@SMAA.fx"));
    assert!(merged.contains("[CAS.fx]\nContrast=0.5"));
    assert!(merged.contains("[DLSS5_Feed.fx]"));
}

#[test]
fn test_feeder_upgrades_outdated_local_dlss_dll_and_restores_original() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("dlss_upgrade");
    let game_dir = temp.join("GameDir");
    let bin_dir = game_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let original_dlss_bytes = b"ANCIENT_DLSS_2_4_2_0_BYTES";
    let local_dlss = bin_dir.join("nvngx_dlss.dll");
    fs::write(&local_dlss, original_dlss_bytes).unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Upgrade Test Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 11".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    let deployed_bytes = fs::read(&local_dlss).unwrap();
    assert_eq!(deployed_bytes, b"MOCK_DLSS_PE_BYTES", "Local DLSS DLL must be upgraded to modern payload bytes");
    assert!(bin_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed beside executable");

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore must succeed");
    assert!(restored, "restore_game must report success");

    let restored_bytes = fs::read(&local_dlss).unwrap();
    assert_eq!(restored_bytes, original_dlss_bytes, "Original DLSS DLL must be restored byte-for-byte");

    assert!(!bin_dir.join("dxgi.dll").exists(), "dxgi.dll hook must be removed on rollback");
    assert!(!bin_dir.join("dlss5-feed.addon64").exists(), "dlss5-feed.addon64 must be removed on rollback");
    assert!(!bin_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be removed on rollback");
}

#[test]
fn test_deploy_opengl_feeder_deploys_opengl32_and_cleans_stale_dxgi() {
    let _guard = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("deploy_opengl");
    let game_dir = temp.join("game");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let stale_dxgi = game_dir.join("dxgi.dll");
    fs::write(&stale_dxgi, b"MOCK_RESHADE_BYTES").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("OpenGL Test Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "OpenGL".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    let hook_path = game_dir.join("opengl32.dll");
    assert!(hook_path.exists(), "opengl32.dll must be deployed for OpenGL titles");
    assert!(!stale_dxgi.exists(), "Stale dxgi.dll must be removed when switching to opengl32.dll");

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore must succeed");
    assert!(restored, "restore_game must report success");
    assert!(!hook_path.exists(), "opengl32.dll must be removed on rollback");
}

#[test]
fn test_hot_swap_between_feeder_and_optiscaler_without_intermediate_restore() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("hotswap");
    let game_dir = temp.join("VulkanGame");
    let bin_dir = game_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("vulkangame.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..110].copy_from_slice(b"vkCreateIn");
    fs::write(&exe_path, &exe_bytes).unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let feeder_opts = DeployOptions {
        game_name: Some("Vulkan Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "Vulkan".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res1 = deploy_feeder_with_bundle(&feeder_opts, &payloads).expect("deploy feeder must succeed");
    assert!(res1.success);

    assert!(bin_dir.join("dxgi.dll").exists());
    assert!(bin_dir.join("ReShade.ini").exists());
    assert!(bin_dir.join("dlss5-feed.addon64").exists());
    assert!(bin_dir.join("renodx-mfgunlock.addon64").exists());
    assert!(bin_dir.join("reshade-shaders").exists());
    assert!(dlss_studio::core::vulkan_layer::is_game_registered(&game_dir));

    let opti_opts = DeployOptions {
        game_name: Some("Vulkan Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "Vulkan".to_string(),
        pre_sr: true,
        passes: 2,
        mfg_unlock: true,
        mfg_multiplier: 4,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res2 = deploy_optiscaler_with_bundle(&opti_opts, &payloads).expect("deploy optiscaler must succeed");
    assert!(res2.success);

    assert!(bin_dir.join("dxgi.dll").exists());
    assert!(bin_dir.join("OptiScaler.ini").exists());
    assert!(!bin_dir.join("RTXMFG-Universal.json").exists(), "DirectX 12 RTXMFG hook must NOT be deployed on Vulkan");

    assert!(!bin_dir.join("ReShade.ini").exists(), "ReShade.ini must be purged when hot-swapping to OptiScaler");
    assert!(!bin_dir.join("ReShadePreset.ini").exists(), "ReShadePreset.ini must be purged");
    assert!(!bin_dir.join("reshadegui.ini").exists(), "reshadegui.ini must be purged");
    assert!(!bin_dir.join("dlss5-feed.addon64").exists(), "dlss5-feed.addon64 must be purged");
    assert!(!bin_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must be purged");
    assert!(!bin_dir.join("reshade-shaders").exists(), "reshade-shaders directory must be purged");
    assert!(!dlss_studio::core::vulkan_layer::is_game_registered(&game_dir), "Vulkan layer must be unregistered when hot-swapping to OptiScaler");

    let res3 = deploy_feeder_with_bundle(&feeder_opts, &payloads).expect("deploy feeder 2 must succeed");
    assert!(res3.success);

    assert!(bin_dir.join("ReShade.ini").exists());
    assert!(bin_dir.join("renodx-mfgunlock.addon64").exists());
    assert!(dlss_studio::core::vulkan_layer::is_game_registered(&game_dir));

    assert!(!bin_dir.join("OptiScaler.ini").exists(), "OptiScaler.ini must be purged when hot-swapping to Feeder");
    assert!(!bin_dir.join("version.dll").exists(), "version.dll must be purged when hot-swapping to Feeder");
    assert!(!bin_dir.join("RTXMFG-Universal.json").exists(), "RTXMFG-Universal.json must be purged");

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);
    assert!(!bin_dir.join("dxgi.dll").exists());
    assert!(!bin_dir.join("ReShade.ini").exists());
    assert!(!bin_dir.join("OptiScaler.ini").exists());
    assert!(!dlss_studio::core::vulkan_layer::is_game_registered(&game_dir));
}

#[test]
fn test_hot_swap_preserves_original_game_files_through_multiple_swaps() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("hotswap_preserve");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"ORIGINAL_EXE_PAYLOAD").unwrap();

    let genuine_dxgi = game_dir.join("dxgi.dll");
    fs::write(&genuine_dxgi, b"GENUINE_GAME_DXGI_CONTENT_12345").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let feeder_opts = DeployOptions {
        game_name: Some("Preserve Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 11".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    deploy_feeder_with_bundle(&feeder_opts, &payloads).unwrap();
    assert_ne!(fs::read(&genuine_dxgi).unwrap(), b"GENUINE_GAME_DXGI_CONTENT_12345");

    let opti_opts = DeployOptions {
        game_name: Some("Preserve Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 11".to_string(),
        pre_sr: true,
        passes: 2,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };
    deploy_optiscaler_with_bundle(&opti_opts, &payloads).unwrap();

    let native_opts = DeployOptions {
        game_name: Some("Preserve Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 11".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };
    deploy_native_dlss5_with_bundle(&native_opts, &payloads).unwrap();

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore must succeed");
    assert!(restored);
    assert_eq!(fs::read(&genuine_dxgi).unwrap(), b"GENUINE_GAME_DXGI_CONTENT_12345", "Original vanilla file must be completely preserved across multiple hot-swaps");
}

#[test]
fn test_feeder_route_32bit_deploys_reshade32_and_addon32_fallback_without_dgvoodoo() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_32bit_fallback");
    let game_dir = temp.join("PsychonautsGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Psychonauts.exe");
    fs::write(&exe_path, create_mock_pe32()).unwrap();

    let pe_info = dlss_studio::core::pe::inspect_pe(&exe_path).expect("Mock PE32 must be valid");
    assert_eq!(pe_info.bitness, 32);

    let mut payloads = create_mock_payload_bundle(&temp.join("payloads"));
    payloads.dgvoodoo = None;

    let opts = DeployOptions {
        game_name: Some("Psychonauts".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 9".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle for 32-bit must succeed");
    assert!(res.success);

    let d3d9_path = game_dir.join("d3d9.dll");
    assert!(d3d9_path.exists(), "d3d9.dll must be deployed");
    assert_eq!(fs::read(&d3d9_path).unwrap(), b"MOCK_RESHADE32_PE_BYTES", "d3d9.dll must be ReShade32.dll in fallback mode");

    assert!(game_dir.join("dlss5-feed.addon32").exists(), "dlss5-feed.addon32 must be deployed for 32-bit process");
    assert!(game_dir.join("dlss5-feed.cfg").exists(), "dlss5-feed.cfg must be deployed");
    assert!(game_dir.join("ReShade.ini").exists(), "ReShade.ini must be deployed");
    assert!(game_dir.join("ReShadePreset.ini").exists(), "ReShadePreset.ini must be deployed");
    assert!(game_dir.join("reshade-shaders").exists(), "reshade-shaders must be deployed");
}

#[test]
fn test_feeder_route_32bit_deploys_dgvoodoo_and_host64_bridge() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_dgvoodoo");
    let game_dir = temp.join("PsychonautsGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Psychonauts.exe");
    fs::write(&exe_path, create_mock_pe32()).unwrap();

    let pe_info = dlss_studio::core::pe::inspect_pe(&exe_path).expect("Mock PE32 must be valid");
    assert_eq!(pe_info.bitness, 32);

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Psychonauts".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 9".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle for 32-bit with dgVoodoo must succeed");
    assert!(res.success);

    let d3d9_path = game_dir.join("d3d9.dll");
    assert!(d3d9_path.exists(), "d3d9.dll must be deployed");
    assert_eq!(fs::read(&d3d9_path).unwrap(), b"MOCK_DGVOODOO_D3D9_X86", "d3d9.dll must be dgVoodoo x86");

    let conf_path = game_dir.join("dgVoodoo.conf");
    assert!(conf_path.exists(), "dgVoodoo.conf must be deployed");
    let conf_str = fs::read_to_string(&conf_path).unwrap();
    assert!(conf_str.contains("dgVoodooWatermark=true") || conf_str.contains("dgVoodooWatermark = true"));
    assert!(conf_str.contains("WatermarkDisplayDuration=3") || conf_str.contains("WatermarkDisplayDuration = 3"));
    assert!(conf_str.contains("3DfxWatermark=true") || conf_str.contains("3DfxWatermark = true"));
    assert!(conf_str.contains("3DfxSplashScreen=false") || conf_str.contains("3DfxSplashScreen = false"));
    assert!(conf_str.contains("VRAM=2048") || conf_str.contains("VRAM = 2048"));

    let dxgi_path = game_dir.join("dxgi.dll");
    assert!(dxgi_path.exists(), "dxgi.dll must be deployed as ReShade hook");
    assert_eq!(fs::read(&dxgi_path).unwrap(), b"MOCK_RESHADE32_PE_BYTES", "dxgi.dll must be ReShade32");

    assert!(game_dir.join("dlss5-feed.addon32").exists(), "dlss5-feed.addon32 must be deployed");
    assert!(game_dir.join("dlss5-feed.cfg").exists(), "dlss5-feed.cfg must be deployed");
    assert!(game_dir.join("ReShade.ini").exists(), "ReShade.ini must be deployed");
    assert!(game_dir.join("ReShadePreset.ini").exists(), "ReShadePreset.ini must be deployed");
    assert!(game_dir.join("reshade-shaders").exists(), "reshade-shaders must be deployed");

    let host64_dir = game_dir.join("host64");
    assert!(host64_dir.is_dir(), "host64/ directory must exist");
    assert!(host64_dir.join("dlss5-feed-host64.exe").exists(), "host64/dlss5-feed-host64.exe must exist");
    assert_eq!(fs::read(host64_dir.join("dlss5-feed-host64.exe")).unwrap(), b"MOCK_FEEDER_HOST64_EXE");
    assert!(host64_dir.join("dxgi.dll").exists(), "host64/dxgi.dll (ReShade64) must exist");
    assert_eq!(fs::read(host64_dir.join("dxgi.dll")).unwrap(), b"MOCK_RESHADE_PE_BYTES");
    assert!(host64_dir.join("renodx-dlss5.addon64").exists(), "host64/renodx-dlss5.addon64 must exist");
    assert!(host64_dir.join("nvngx_dlssnr.dll").exists(), "host64/nvngx_dlssnr.dll must exist");
    assert!(host64_dir.join("nvngx_dlss.dll").exists(), "host64/nvngx_dlss.dll must exist");
    assert!(host64_dir.join("ReShade.ini").exists(), "host64/ReShade.ini must exist");

    assert!(!game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must NOT be in 32-bit game root");
    assert!(!game_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must NOT be in 32-bit game root");
    assert!(dlss_studio::core::pe::is_large_address_aware(&exe_path), "Target 32-bit executable must be patched with LAA (4GB patch)");

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("Restore must succeed");
    assert!(restored, "Restore must report true");
    assert!(!game_dir.join("d3d9.dll").exists(), "d3d9.dll must be removed on restore");
    assert!(!game_dir.join("dgVoodoo.conf").exists(), "dgVoodoo.conf must be removed on restore");
    assert!(!game_dir.join("dxgi.dll").exists(), "dxgi.dll must be removed on restore");
    assert!(!game_dir.join("host64").exists(), "host64/ must be removed on restore");
    assert!(!game_dir.join("dlss5-feed.addon32").exists(), "dlss5-feed.addon32 must be removed on restore");
    assert!(!dlss_studio::core::pe::is_large_address_aware(&exe_path), "Target 32-bit executable must have original non-LAA restored from vanilla backup");
}

#[test]
fn test_feeder_route_d3d8_deploys_dgvoodoo_d3d8() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_d3d8");
    let game_dir = temp.join("LegacyD3D8Game");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("GameD3D8.exe");
    fs::write(&exe_path, create_mock_pe32()).unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Legacy D3D8 Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 8".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle for D3D8 must succeed");
    assert!(res.success);

    let d3d8_path = game_dir.join("d3d8.dll");
    assert!(d3d8_path.exists(), "d3d8.dll must be deployed");
    assert_eq!(fs::read(&d3d8_path).unwrap(), b"MOCK_DGVOODOO_D3D8_X86", "d3d8.dll must be dgVoodoo D3D8");

    let dxgi_path = game_dir.join("dxgi.dll");
    assert!(dxgi_path.exists(), "dxgi.dll must be deployed");
    assert_eq!(fs::read(&dxgi_path).unwrap(), b"MOCK_RESHADE32_PE_BYTES");

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("Restore must succeed");
    assert!(restored);
    assert!(!game_dir.join("d3d8.dll").exists());
    assert!(!game_dir.join("dxgi.dll").exists());
    assert!(!game_dir.join("dgVoodoo.conf").exists());
}

#[test]
fn test_feeder_route_64bit_deploys_reshade64_and_addon64() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("feeder_64bit");
    let game_dir = temp.join("CyberpunkGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Cyberpunk2077.exe");
    fs::write(&exe_path, create_mock_pe64()).unwrap();

    let pe_info = dlss_studio::core::pe::inspect_pe(&exe_path).expect("Mock PE64 must be valid");
    assert_eq!(pe_info.bitness, 64);

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Cyberpunk 2077".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle for 64-bit must succeed");
    assert!(res.success);

    let dxgi_path = game_dir.join("dxgi.dll");
    assert!(dxgi_path.exists(), "dxgi.dll must be deployed");
    assert_eq!(fs::read(&dxgi_path).unwrap(), b"MOCK_RESHADE_PE_BYTES", "dxgi.dll must be ReShade64.dll");

    assert!(game_dir.join("dlss5-feed.addon64").exists(), "dlss5-feed.addon64 must be deployed for 64-bit process");
    assert!(game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be deployed in 64-bit game");
    assert!(game_dir.join("nvngx_dlssnr.dll").exists(), "nvngx_dlssnr.dll must be deployed in 64-bit game");
    assert!(game_dir.join("nvngx_dlss.dll").exists(), "nvngx_dlss.dll must be deployed in 64-bit game");

    assert!(!game_dir.join("dlss5-feed.addon32").exists(), "dlss5-feed.addon32 must NOT be deployed in 64-bit game");

    let manifest = dlss_studio::core::journal::read_manifest(&game_dir).expect("Active manifest must exist");
    assert_eq!(manifest.game.as_ref().and_then(|g| g.bitness), Some(64), "Manifest must record 64-bit architecture");
}

#[test]
fn test_nr_style_configuration_across_all_routes() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("nr_style");
    let game_dir = temp.join("TestGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, create_mock_pe64()).unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let native_opts = DeployOptions {
        game_name: Some("Test Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 2,
        nr_style_enabled: true,
    };
    let res_native = deploy_native_dlss5_with_bundle(&native_opts, &payloads).expect("Native deploy must succeed");
    assert!(res_native.success);

    let reshade_ini = fs::read_to_string(game_dir.join("ReShade.ini")).expect("ReShade.ini must exist");
    assert_eq!(get_ini(&reshade_ini, "RenoDX.DLSS5", "NRStyle"), Some("2".to_string()), "Native route must write NRStyle=2");

    let scanned = dlss_studio::core::scan::scan_game_directory(&game_dir).expect("Scan must find deployed game");
    assert_eq!(scanned.nr_style, 2, "Scanner must detect nr_style = 2 from ReShade.ini");
    assert!(scanned.nr_style_enabled, "Scanner must detect nr_style_enabled = true");

    let feeder_opts = DeployOptions {
        game_name: Some("Test Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 11".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 1,
        nr_style_enabled: true,
    };
    let res_feeder = deploy_feeder_with_bundle(&feeder_opts, &payloads).expect("Feeder deploy must succeed");
    assert!(res_feeder.success);

    let feeder_reshade_ini = fs::read_to_string(game_dir.join("ReShade.ini")).expect("ReShade.ini must exist for Feeder");
    assert_eq!(get_ini(&feeder_reshade_ini, "RenoDX.DLSS5", "NRStyle"), Some("1".to_string()), "Feeder route must write NRStyle=1");

    let opti_opts = DeployOptions {
        game_name: Some("Test Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 3,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 2,
        nr_style_enabled: true,
    };
    let res_opti = deploy_optiscaler_with_bundle(&opti_opts, &payloads).expect("OptiScaler deploy must succeed");
    assert!(res_opti.success);

    let opti_ini = fs::read_to_string(game_dir.join("OptiScaler.ini")).expect("OptiScaler.ini must exist");
    assert_eq!(get_ini(&opti_ini, "DlssNr", "Style"), Some("2".to_string()), "OptiScaler route must write DlssNr Style=2");
}

#[test]
fn test_native_dlss5_configures_direct_nr_hooks_and_uplift() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("direct_nr");
    let game_dir = temp.join("TestGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, create_mock_pe64()).unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let native_opts = DeployOptions {
        game_name: Some("Direct NR Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 2,
        nr_style_enabled: true,
    };

    let res = deploy_native_dlss5_with_bundle(&native_opts, &payloads).expect("deploy_native_dlss5_with_bundle must succeed");
    assert!(res.success);

    let reshade_ini = fs::read_to_string(game_dir.join("ReShade.ini")).expect("ReShade.ini must exist");
    assert_eq!(get_ini(&reshade_ini, "DLSS_NR", "Passes"), Some("1".to_string()), "Native route must write DLSS_NR Passes=1");
    assert_eq!(get_ini(&reshade_ini, "DLSS_NR", "ResolutionMode"), Some("0".to_string()), "Native route must write DLSS_NR ResolutionMode=0");
    assert_eq!(get_ini(&reshade_ini, "DLSS_NR", "PreSR"), Some("0".to_string()), "Native route must write DLSS_NR PreSR=0");
    assert_eq!(get_ini(&reshade_ini, "RenoDX.DLSS5", "EnableHooks"), Some("2".to_string()), "Native route must write EnableHooks=2");
    assert_eq!(get_ini(&reshade_ini, "RenoDX.DLSS5", "NeuralUplift"), Some("1".to_string()), "Native route must write NeuralUplift=1");
    assert_eq!(get_ini(&reshade_ini, "RenoDX.DLSS5", "NRAutoMask"), Some("1".to_string()), "Native route must write NRAutoMask=1");
    assert_eq!(get_ini(&reshade_ini, "RenoDX.DLSS5", "NRStyle"), Some("2".to_string()), "Native route must write NRStyle=2");
}

#[test]
fn test_native_dlss5_deploys_renodx_addon_and_cleans_conflicting_older_addons() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("renodx_deploy");
    let game_dir = temp.join("TestGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, create_mock_pe64()).unwrap();

    // Pre-populate game folder with conflicting older addons
    let stale_nr = game_dir.join("dlss-nr.addon64");
    fs::write(&stale_nr, b"OLD_STALE_NR").unwrap();
    let stale_dlss = game_dir.join("renodx-dlss.addon64");
    fs::write(&stale_dlss, b"OLD_STALE_DLSS").unwrap();

    let mut payloads = create_mock_payload_bundle(&temp.join("payloads"));
    let renodx_addon = temp.join("payloads").join("renodx-dlss5.addon64");
    fs::write(&renodx_addon, b"MOCK_RENODX_DLSS5_ADDON").unwrap();
    payloads.renodx_dlss5_addon = Some(renodx_addon);

    let native_opts = DeployOptions {
        game_name: Some("Direct NR Stacking Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 2,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 1,
        nr_style_enabled: true,
    };

    let res = deploy_native_dlss5_with_bundle(&native_opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    // renodx-dlss5.addon64 must be deployed
    assert!(game_dir.join("renodx-dlss5.addon64").is_file(), "renodx-dlss5.addon64 must be deployed");

    // Conflicting older/prototype addons must be automatically cleaned up
    assert!(!game_dir.join("dlss-nr.addon64").exists(), "Stale dlss-nr.addon64 must be cleaned up");
    assert!(!game_dir.join("renodx-dlss.addon64").exists(), "Stale renodx-dlss.addon64 must be cleaned up");
}

#[test]
fn test_native_dlss5_mfg_unlock_upgrades_nvngx_dlssg_and_restores() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("dlssg_upgrade");
    let game_dir = temp.join("TestGame");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, create_mock_pe64()).unwrap();

    let original_dlssg = game_dir.join("nvngx_dlssg.dll");
    fs::write(&original_dlssg, b"ORIGINAL_LEGACY_DLSSG_3.1.1").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let native_opts = DeployOptions {
        game_name: Some("DLSS-G Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 2,
        nr_style: 1,
        nr_style_enabled: true,
    };

    let res = deploy_native_dlss5_with_bundle(&native_opts, &payloads).expect("deploy must succeed");
    assert!(res.success);

    assert!(game_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must be deployed");
    let upgraded_bytes = fs::read(&original_dlssg).expect("nvngx_dlssg.dll must exist");
    assert_eq!(upgraded_bytes, b"MOCK_NVNGX_DLSSG", "nvngx_dlssg.dll must be upgraded to modern version");

    let manifest = dlss_studio::core::journal::read_manifest(&game_dir).expect("Active manifest must exist");
    assert!(manifest.replaced.iter().any(|r| r.rel.ends_with("nvngx_dlssg.dll")), "Manifest must track nvngx_dlssg.dll replacement");
    let prefix = manifest.backup_prefix.as_deref().unwrap_or("");
    let backup_dlssg = game_dir.join("_DLSS5_Backup").join(prefix).join("nvngx_dlssg.dll");
    assert!(backup_dlssg.exists(), "Original legacy nvngx_dlssg.dll must be backed up");
    assert_eq!(fs::read(&backup_dlssg).unwrap(), b"ORIGINAL_LEGACY_DLSSG_3.1.1");

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore_game must succeed");
    assert!(restored, "restore_game must return true");

    let restored_bytes = fs::read(&original_dlssg).expect("nvngx_dlssg.dll must still exist");
    assert_eq!(restored_bytes, b"ORIGINAL_LEGACY_DLSSG_3.1.1", "Original legacy nvngx_dlssg.dll must be restored");

    assert!(!game_dir.join("renodx-mfgunlock.addon64").exists(), "renodx-mfgunlock.addon64 must be removed on restore");
    assert!(!game_dir.join("dxgi.dll").exists(), "dxgi.dll must be removed on restore");
    assert!(!game_dir.join("ReShade.ini").exists(), "ReShade.ini must be removed on restore");
    assert!(!game_dir.join("_DLSS5_Backup").join("manifest.json").exists(), "Active manifest.json must not exist after restore");
}

#[test]
fn test_dlss5_d3d12_fix_deployed_with_native_route_and_cleaned_on_restore() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("dlss5_mip_fix_native");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Resonance.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Resonance: A Plague Tale Legacy".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_native_dlss5_with_bundle(&opts, &payloads).expect("deploy_native_dlss5_with_bundle must succeed");
    assert!(res.success);

    assert!(game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be deployed");
    assert!(game_dir.join("dlss-mip-fix.addon64").exists(), "dlss-mip-fix.addon64 must be deployed as companion");

    fs::write(game_dir.join("dlss-mip-fix.cfg"), b"fix=1\n").unwrap();
    fs::write(game_dir.join("dlss-mip-fix.log"), b"DLSS Studio D3D12 Mip Companion active\n").unwrap();

    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore_game must succeed");
    assert!(restored, "restore_game must return true");

    assert!(!game_dir.join("dlss-mip-fix.addon64").exists(), "dlss-mip-fix.addon64 must be removed on restore");
    assert!(!game_dir.join("dlss-mip-fix.cfg").exists(), "dlss-mip-fix.cfg must be removed on restore");
    assert!(!game_dir.join("dlss-mip-fix.log").exists(), "dlss-mip-fix.log must be removed on restore");
    assert!(!game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be removed on restore");
}

#[test]
fn test_dlss5_d3d12_fix_deployed_with_feeder_route() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("dlss5_mip_fix_feeder");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let opts = DeployOptions {
        game_name: Some("Feeder Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 2,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res = deploy_feeder_with_bundle(&opts, &payloads).expect("deploy_feeder_with_bundle must succeed");
    assert!(res.success);

    assert!(game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must be deployed");
    assert!(game_dir.join("dlss-mip-fix.addon64").exists(), "dlss-mip-fix.addon64 must be deployed in Feeder route");
}

#[test]
fn test_hot_swap_removes_dlss5_d3d12_fix_when_switching_to_optiscaler() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("dlss5_mip_fix_hotswap");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, b"DUMMY_GAME_EXE").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    let native_opts = DeployOptions {
        game_name: Some("HotSwap Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };

    let res1 = deploy_native_dlss5_with_bundle(&native_opts, &payloads).expect("deploy native must succeed");
    assert!(res1.success);
    assert!(game_dir.join("dlss-mip-fix.addon64").exists());
    fs::write(game_dir.join("dlss-mip-fix.cfg"), b"mock").unwrap();
    fs::write(game_dir.join("dlss-mip-fix.log"), b"mock").unwrap();

    let opti_opts = DeployOptions {
        game_name: Some("HotSwap Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 2,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 1,
        nr_style_enabled: true,
    };

    let res2 = deploy_optiscaler_with_bundle(&opti_opts, &payloads).expect("deploy optiscaler must succeed");
    assert!(res2.success);

    assert!(game_dir.join("OptiScaler.ini").exists());
    assert!(!game_dir.join("dlss-mip-fix.addon64").exists(), "dlss-mip-fix.addon64 must NOT exist in OptiScaler route");
    assert!(!game_dir.join("dlss-mip-fix.cfg").exists(), "dlss-mip-fix.cfg must NOT exist in OptiScaler route");
    assert!(!game_dir.join("dlss-mip-fix.log").exists(), "dlss-mip-fix.log must NOT exist in OptiScaler route");
    assert!(!game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 must NOT exist in OptiScaler route");
}

#[test]
fn test_frame_generation_status_vulkan_unsupported() {
    let mut game = dlss_studio::core::scan::GameEntry::default();
    game.name = "Baldur's Gate 3".to_string();
    game.api = "Vulkan".to_string();
    game.bitness = 64;
    game.dlss_version = Some("3.10.8.0".to_string());
    game.has_frame_generation = false;
    game.can_inject_fg = false;

    let (status, is_on) = dlss_studio::core::install_routes::frame_generation_status(&game);
    assert_eq!(status, "Unsupported (Vulkan API)");
    assert!(!is_on);
}

#[test]
fn test_frame_generation_status_directx11_unsupported() {
    let mut game = dlss_studio::core::scan::GameEntry::default();
    game.name = "Baldur's Gate 3 (DX11)".to_string();
    game.api = "DirectX 11".to_string();
    game.bitness = 64;
    game.has_frame_generation = false;
    game.can_inject_fg = false;

    let (status, is_on) = dlss_studio::core::install_routes::frame_generation_status(&game);
    assert_eq!(status, "Unsupported (DirectX 11)");
    assert!(!is_on);
}

#[test]
fn test_backup_dir_ancestor_resolution() {
    let temp = TempDir::new("ancestor_backup");
    let root = temp.path();
    let deep_dir = root.join("Content").join("NewMoon").join("Binaries").join("WinGDK");
    fs::create_dir_all(&deep_dir).unwrap();

    let backup = root.join("_DLSS5_Backup");
    fs::create_dir_all(&backup).unwrap();
    fs::write(backup.join("manifest.json"), b"{}").unwrap();

    let resolved = dlss_studio::core::journal::backup_dir(&deep_dir);
    assert!(
        resolved == backup
            || fs::canonicalize(&resolved).ok() == fs::canonicalize(&backup).ok(),
        "backup_dir must resolve ancestor _DLSS5_Backup from nested game dir (resolved: {:?}, expected: {:?})",
        resolved,
        backup
    );
}

#[test]
fn test_retroactive_backfill_added_history() {
    let temp = TempDir::new("backfill_test");
    let game_dir = temp.join("GameDir");
    let bdir = game_dir.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    let mut m1 = dlss_studio::core::journal::ActiveManifest::default();
    m1.added = vec!["file_a.dll".to_string(), "file_b.dll".to_string()];
    fs::write(bdir.join("manifest.json.done-100"), serde_json::to_vec(&m1).unwrap()).unwrap();

    let mut m2 = dlss_studio::core::journal::ActiveManifest::default();
    m2.added = vec!["nvngx_dlss.dll".to_string()];
    fs::write(bdir.join("manifest.json.done-200"), serde_json::to_vec(&m2).unwrap()).unwrap();

    let mut m3 = dlss_studio::core::journal::ActiveManifest::default();
    m3.added = vec!["file_c.dll".to_string()];
    fs::write(bdir.join("manifest.json.done-300"), serde_json::to_vec(&m3).unwrap()).unwrap();

    assert!(!bdir.join("added_history.json").exists(), "added_history.json must not exist initially");

    let history = dlss_studio::core::journal::read_or_backfill_added_history(&game_dir);
    assert!(bdir.join("added_history.json").exists(), "added_history.json must be persisted after backfill");
    assert!(history.files.contains("file_a.dll"));
    assert!(history.files.contains("file_b.dll"));
    assert!(history.files.contains("nvngx_dlss.dll"));
    assert!(history.files.contains("file_c.dll"));
}

#[test]
fn test_cumulative_added_history_multi_cycle_swap_and_prune() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("cumulative_cycle_test");
    let game_dir = temp.join("GameDir");
    fs::create_dir_all(&game_dir).unwrap();

    let exe_path = game_dir.join("Game.exe");
    fs::write(&exe_path, create_mock_pe64()).unwrap();

    // Genuine game file that should be backed up and preserved
    let original_dxgi = game_dir.join("dxgi.dll");
    fs::write(&original_dxgi, b"GENUINE_GAME_DXGI").unwrap();

    let payloads = create_mock_payload_bundle(&temp.join("payloads"));

    // Cycle 1: Native DLSS 5 route
    let native_opts = DeployOptions {
        game_name: Some("MultiCycle Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };
    let res1 = deploy_native_dlss5_with_bundle(&native_opts, &payloads).expect("deploy native");
    assert!(res1.success);
    assert!(game_dir.join("renodx-dlss5.addon64").exists());

    // Cycle 2: Feeder route (adds nvngx_dlss.dll)
    let feeder_opts = DeployOptions {
        game_name: Some("MultiCycle Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: false,
        passes: 1,
        mfg_unlock: true,
        mfg_multiplier: 2,
        nr_style: 0,
        nr_style_enabled: false,
    };
    let res2 = deploy_feeder_with_bundle(&feeder_opts, &payloads).expect("deploy feeder");
    assert!(res2.success);
    assert!(game_dir.join("nvngx_dlss.dll").exists());

    // Cycle 3: OptiScaler route
    let opti_opts = DeployOptions {
        game_name: Some("MultiCycle Game".to_string()),
        game_dir: game_dir.clone(),
        exe_path: exe_path.clone(),
        api: "DirectX 12".to_string(),
        pre_sr: true,
        passes: 1,
        mfg_unlock: false,
        mfg_multiplier: 1,
        nr_style: 0,
        nr_style_enabled: false,
    };
    let res3 = deploy_optiscaler_with_bundle(&opti_opts, &payloads).expect("deploy optiscaler");
    assert!(res3.success);

    // Cycles 4..10: Simulate 7 additional swaps/deployments, exceeding the 5-manifest prune limit
    let bdir = game_dir.join("_DLSS5_Backup");
    for i in 4..=10 {
        let mut sim_manifest = dlss_studio::core::journal::ActiveManifest::default();
        sim_manifest.route = format!("variant_{}", i);
        let extra_file = format!("extra_mod_{}.dll", i);
        fs::write(game_dir.join(&extra_file), b"SIMULATED_MOD").unwrap();
        sim_manifest.added = vec![extra_file];

        // Archive previous manifest if exists, carrying forward genuine replaced backups
        let mpath = bdir.join("manifest.json");
        if mpath.exists() {
            if let Some(prev) = dlss_studio::core::journal::read_manifest(&game_dir) {
                sim_manifest.replaced = prev.replaced;
                sim_manifest.backup_prefix = prev.backup_prefix;
            }
            let done_path = bdir.join(format!("manifest.json.done-1000{}", i));
            let _ = fs::rename(&mpath, done_path);
        }
        dlss_studio::core::journal::save_manifest(&game_dir, &sim_manifest).unwrap();
    }

    // Assert pruning happened: only top 5 done manifests exist
    let done_count = fs::read_dir(&bdir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("manifest.json.done-"))
        .count();
    assert_eq!(done_count, 5, "Only 5 done manifests should be retained after pruning");

    // But cumulative added_history.json still remembers ALL files from cycles 1 and 2!
    let history = dlss_studio::core::journal::read_added_history(&game_dir).expect("added_history.json must exist");
    assert!(history.files.contains("renodx-dlss5.addon64"), "History must retain cycle 1 files");
    assert!(history.files.contains("nvngx_dlss.dll"), "History must retain cycle 2 nvngx_dlss.dll");
    assert!(history.files.contains("extra_mod_4.dll"), "History must retain cycle 4 files");
    assert!(history.files.contains("extra_mod_10.dll"), "History must retain cycle 10 files");

    // Now execute restore_game - it must clean 100% of all added files across all 10 cycles
    let restored = dlss_studio::core::journal::restore_game(&game_dir).expect("restore_game");
    assert!(restored);

    // Assert zero leftover files!
    assert!(!game_dir.join("nvngx_dlss.dll").exists(), "nvngx_dlss.dll from cycle 2 MUST be purged");
    assert!(!game_dir.join("renodx-dlss5.addon64").exists(), "renodx-dlss5.addon64 from cycle 1 MUST be purged");
    for i in 4..=10 {
        let extra_file = format!("extra_mod_{}.dll", i);
        assert!(!game_dir.join(&extra_file).exists(), "{} must be purged on restore", extra_file);
    }

    // Genuine original dxgi.dll must be intact!
    let dxgi_bytes = fs::read(&original_dxgi).expect("original dxgi.dll must exist");
    assert_eq!(dxgi_bytes, b"GENUINE_GAME_DXGI", "Original dxgi.dll must be restored");

    // added_history.json must be archived
    assert!(!bdir.join("added_history.json").exists(), "added_history.json must be archived on restore");
}


use dlss_studio::core::payloads::discovery::*;

#[test]
fn test_find_optiscaler_payload_runs_safely() {
    let _ = find_optiscaler_payload();
}

#[test]
fn test_get_component_roots_not_empty() {
    let roots = get_component_roots();
    assert!(!roots.is_empty());
}

#[test]
fn test_find_ancillary_payloads_run_safely() {
    let _ = find_reshade64_payload();
    let _ = find_reshade32_payload();
    let _ = find_mfg_addon_payload();
    let _ = find_renodx_payload();
    let _ = find_dlss5_d3d12_fix_payload();
    let _ = find_dlss_payload();
    let _ = find_dlssnr_payload();
    let _ = find_standalone_mfg_payload();
    let _ = find_streamline_payload();
    let _ = find_overlay_addon_payload();
}

#[test]
fn test_real_system_component_discovery() {
    let opti = find_optiscaler_payload();
    let r64 = find_reshade64_payload();
    let r32 = find_reshade32_payload();
    let renodx = find_renodx_payload();
    let mip = find_dlss5_d3d12_fix_payload();
    let mfg = find_mfg_addon_payload();
    let dlss = find_dlss_payload();
    let dlssnr = find_dlssnr_payload();
    let standalone = find_standalone_mfg_payload();
    let sl = find_streamline_payload();
    let overlay = find_overlay_addon_payload();

    println!("optiscaler: {:?}", opti);
    println!("reshade64: {:?}", r64);
    println!("reshade32: {:?}", r32);
    println!("renodx: {:?}", renodx);
    println!("dlss5_mip_fix: {:?}", mip);
    println!("mfg_addon: {:?}", mfg);
    println!("dlss: {:?}", dlss);
    println!("dlssnr: {:?}", dlssnr);
    println!("standalone_mfg: {:?}", standalone);
    println!("streamline: {:?}", sl);
    println!("overlay: {:?}", overlay);

    // If components are present in system/test data, verify discovered types
    if let Some(r64_p) = r64 {
        assert!(r64_p.is_file());
    }
    if let Some(opti_p) = opti {
        assert!(opti_p.is_dir());
    }
}



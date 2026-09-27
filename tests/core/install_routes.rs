use dlss_studio::core::install_routes::*;
use dlss_studio::core::scan::{GameEntry, GameFileItem};
use dlss_studio::core::journal::ActiveManifest;
use crate::common::TempDir;
use std::path::PathBuf;

fn make_test_game(api: &str, bitness: u32, dlss: Option<&str>) -> GameEntry {
    GameEntry {
        name: "Test Game".to_string(),
        dir: PathBuf::from("C:\\Games\\Test"),
        exe_path: PathBuf::from("C:\\Games\\Test\\game.exe"),
        exe_rel: "game.exe".to_string(),
        bitness,
        api: api.to_string(),
        dlss_version: dlss.map(|s| s.to_string()),
        optiscaler_installed: false,
        optiscaler_presr: false,
        optiscaler_passes: 1,
        has_frame_generation: false,
        can_inject_fg: false,
        mfg_unlock_installed: false,
        has_backup: false,
        launcher: "Steam".to_string(),
        poster: None,
        reshade_installed: false,
        reshade_version: None,
        reshade_addon_support: false,
        addon_installed: false,
        installed_route: None,
        files: Vec::new(),
        available_exes: Vec::new(),
        is_laa: false,
        nr_style: 0,
        nr_style_enabled: false,
        mfg_multiplier: 4,
        has_anti_cheat: false,
    }
}

#[test]
fn test_32bit_game_rejects_optiscaler() {
    let g = make_test_game("DirectX 12", 32, Some("3.7.0"));
    assert_eq!(check_opti_reason(&g), Some(OptiReason::Unsupported));
    let routes = routes_for(&g);
    assert!(!routes.contains(&InstallRoute::OptiScaler));
    assert_eq!(routes, vec![InstallRoute::Feeder]);
}

#[test]
fn test_game_without_dlss_rejects_optiscaler() {
    let g = make_test_game("DirectX 12", 64, None);
    assert_eq!(check_opti_reason(&g), Some(OptiReason::NeedsDlss));
    let routes = routes_for(&g);
    assert!(!routes.contains(&InstallRoute::OptiScaler));
    assert_eq!(routes, vec![InstallRoute::Feeder]);
}

#[test]
fn test_64bit_game_with_dlss_allows_optiscaler() {
    let g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    assert_eq!(check_opti_reason(&g), None);
    let routes = routes_for(&g);
    assert!(routes.contains(&InstallRoute::OptiScaler));
}

#[test]
fn test_dx9_game_routing() {
    let g = make_test_game("DirectX 9", 32, None);
    let routes = routes_for(&g);
    assert_eq!(routes, vec![InstallRoute::Feeder]);
    assert_eq!(recommended_route(&g), InstallRoute::Feeder);
}

#[test]
fn test_dx8_game_routing() {
    let g = make_test_game("DirectX 8", 32, None);
    let routes = routes_for(&g);
    assert_eq!(routes, vec![InstallRoute::Feeder]);
    assert_eq!(recommended_route(&g), InstallRoute::Feeder);
}

#[test]
fn test_vulkan_with_dlss_routing() {
    let g = make_test_game("Vulkan", 64, Some("3.7.0"));
    let routes = routes_for(&g);
    assert!(routes.contains(&InstallRoute::OptiScaler));
    assert!(routes.contains(&InstallRoute::Feeder));
    assert!(!routes.contains(&InstallRoute::Native));
    assert_eq!(recommended_route(&g), InstallRoute::Feeder);
}

#[test]
fn test_dx12_game_with_dlss_allows_all() {
    let g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    assert_eq!(check_opti_reason(&g), None);
    let routes = routes_for(&g);
    assert!(routes.contains(&InstallRoute::Native));
    assert!(routes.contains(&InstallRoute::Feeder));
    assert!(routes.contains(&InstallRoute::OptiScaler));
    assert_eq!(recommended_route(&g), InstallRoute::Native);
    assert!(is_native_dlss_supported(&g));
}

#[test]
fn test_vulkan_and_dx11_games_reject_native_dlss() {
    let g_vk = make_test_game("Vulkan", 64, Some("2.4.2"));
    assert!(!is_native_dlss_supported(&g_vk), "Vulkan games cannot use D3D12 Native DLSS");

    let g_dx11 = make_test_game("DirectX 11", 64, Some("2.4.2"));
    assert!(!is_native_dlss_supported(&g_dx11), "DX11 games cannot use D3D12 Native DLSS");

    let g_nodlss = make_test_game("DirectX 12", 64, None);
    assert!(!is_native_dlss_supported(&g_nodlss), "Games without DLSS cannot use Native DLSS");
}

#[test]
fn test_install_route_and_opti_reason_methods() {
    assert_eq!(InstallRoute::Native.as_str(), "native");
    assert_eq!(InstallRoute::OptiScaler.as_str(), "optiscaler");
    assert_eq!(InstallRoute::Feeder.as_str(), "feeder");

    assert!(OptiReason::Unsupported.message().contains("64-bit"));
    assert!(OptiReason::NeedsDlss.message().contains("original DLSS"));
}

#[test]
fn test_frame_generation_status_evaluations() {
    let mut g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    g.has_frame_generation = true;
    assert!(is_frame_generation_supported(&g));
    let (label, on) = frame_generation_status(&g);
    assert_eq!(label, "Active (Native DLSS-G)");
    assert!(on);

    g.has_frame_generation = false;
    g.mfg_unlock_installed = true;
    assert!(is_frame_generation_supported(&g));
    let (label, on) = frame_generation_status(&g);
    assert_eq!(label, "Active (Injected 4x MFG)");
    assert!(on);

    g.mfg_unlock_installed = false;
    g.can_inject_fg = true;
    assert!(is_frame_generation_supported(&g));
    let (label, on) = frame_generation_status(&g);
    assert_eq!(label, "Injectable (Streamline FG)");
    assert!(!on);

    g.can_inject_fg = false;
    g.mfg_unlock_installed = false;

    assert!(!is_frame_generation_supported(&g), "Pure raster game without DLSS must NOT support frame generation");
    let (label, on) = frame_generation_status(&g);
    assert_eq!(label, "Unsupported (Requires Native DLSS-G)");
    assert!(!on);
}

#[test]
fn test_switching_between_vulkan_and_dx11_updates_frame_generation_support() {
    let mut g = make_test_game("Vulkan", 64, Some("2.4.2"));
    g.can_inject_fg = false;
    g.has_frame_generation = false;

    // Vulkan must strictly reject frame generation
    assert!(!is_frame_generation_supported(&g), "Vulkan game with DLSS must NOT support frame generation");

    // DX11 with native DLSS is supported via OptiScaler D3D11on12 bridge
    g.api = "DirectX 11".to_string();
    assert!(is_frame_generation_supported(&g), "DirectX 11 executable with native DLSS must support frame generation via D3D11on12 bridge");

    // DX11 WITHOUT native DLSS must strictly reject frame generation
    g.dlss_version = None;
    assert!(!is_frame_generation_supported(&g), "DirectX 11 executable without native DLSS must strictly reject frame generation");

    g.api = "Vulkan".to_string();
    g.dlss_version = Some("2.4.2".to_string());
    assert!(!is_frame_generation_supported(&g), "Vulkan executable must strictly reject frame generation");
}

#[test]
fn test_route_advisories_for_dead_space_dx9_32bit() {
    let g = make_test_game("DirectX 9", 32, None);
    
    let opti_adv = get_optiscaler_advisory(&g).expect("OptiScaler should have advisory on 32-bit DX9 without DLSS");
    assert!(opti_adv.reasons.iter().any(|r| r.contains("32-bit")));
    assert!(opti_adv.reasons.iter().any(|r| r.contains("DirectX 9") || r.contains("Legacy")));
    assert!(opti_adv.reasons.iter().any(|r| r.contains("Missing Native DLSS")));

    let nat_adv = get_native_dlss_advisory(&g).expect("Native DLSS should have advisory on 32-bit DX9 without DLSS");
    assert!(nat_adv.reasons.iter().any(|r| r.contains("DirectX 12 Required")));
    assert!(nat_adv.reasons.iter().any(|r| r.contains("32-bit")));

    let mfg_adv = get_mfg_advisory(&g, true).expect("MFG should have advisory on game without native DLSS-G");
    assert!(mfg_adv.reasons.iter().any(|r| r.contains("Unsupported API") || r.contains("Missing Native DLSS-G")));
}

#[test]
fn test_route_advisories_none_for_ideal_dx12_game() {
    let mut g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    g.has_frame_generation = true;
    
    assert!(get_optiscaler_advisory(&g).is_none());
    assert!(get_native_dlss_advisory(&g).is_none());
    assert!(get_mfg_advisory(&g, true).is_none());
}

#[test]
fn test_control_dx12_has_no_mfg_advisory() {
    let mut g = make_test_game("DirectX 12", 64, Some("310.2.1.0"));
    g.name = "Control Ultimate Edition".to_string();
    g.exe_rel = "Control_DX12.exe".to_string();
    g.has_frame_generation = false;
    g.can_inject_fg = true;

    assert!(get_mfg_advisory(&g, true).is_none(), "64-bit DirectX 12 game with DLSS and can_inject_fg must NOT show MFG advisory");
    let (label, is_on) = frame_generation_status(&g);
    assert_eq!(label, "Injectable (Streamline FG)");
    assert!(!is_on);
    assert!(is_frame_generation_supported(&g), "is_frame_generation_supported helper must return true for Control_DX12");
}

#[test]
fn test_mfg_advisory_persists_on_incompatible_api_even_when_mfg_unlock_installed() {
    let mut g = make_test_game("DirectX 11", 64, None);
    g.name = "Generic DX11 Game Without Upscaler".to_string();
    g.has_frame_generation = false;
    g.mfg_unlock_installed = true;

    let adv = get_mfg_advisory(&g, true).expect("MFG advisory must still be reported on DirectX 11 when game lacks native DLSS");
    assert!(adv.reasons.iter().any(|r| r.contains("DirectX 11 Limitation")), "Must report DirectX 11 Limitation when lacking DLSS");
}

#[test]
fn test_mfg_advisory_info_notice_for_bg3_dx11_with_native_dlss() {
    let mut g = make_test_game("DirectX 11", 64, Some("3.7.0"));
    g.name = "Baldurs Gate 3".to_string();
    g.files.push(GameFileItem {
        rel: "bin\\nvngx_dlss.dll".to_string(),
        version: Some("3.7.0".to_string()),
    });
    g.has_frame_generation = false;
    g.can_inject_fg = true;

    let adv = get_mfg_advisory(&g, true).expect("BG3 DX11 with native DLSS must show Info interop notice for in-game DLSS on RTX 40-series");
    assert_eq!(adv.severity, AdvisorySeverity::Info, "Advisory must be Info (non-blocking Configuration Notice)");
    assert!(adv.reasons.iter().any(|r| r.contains("DirectX 11 Interop Notice")), "Must state DirectX 11 Interop Notice");
    assert!(is_frame_generation_supported(&g), "is_frame_generation_supported must return true for BG3 DX11");
    let (status, _) = frame_generation_status(&g);
    assert_eq!(status, "Injectable (D3D11on12 DLSS MFG)");
}

#[test]
fn test_optiscaler_dx11_without_upscaler_warns_high_incompatibility() {
    let g = make_test_game("DirectX 11", 64, None);
    let adv = get_optiscaler_advisory(&g).expect("OptiScaler must have advisory on pure DX11 without native upscalers");
    assert_eq!(adv.severity, AdvisorySeverity::Warning);
    assert!(adv.reasons.iter().any(|r| r.contains("Non-DirectX 12 / Missing Native Upscaler")));
    assert!(adv.recommendation.contains("DLSS 5 Feeder"));
}

#[test]
fn test_optiscaler_dx11_with_native_upscaler_gives_interop_notice() {
    let mut g = make_test_game("DirectX 11", 64, None);
    g.files.push(GameFileItem {
        rel: "libxess.dll".to_string(),
        version: Some("1.3.0".to_string()),
    });
    assert!(g.has_native_upscaler(), "Must detect native XeSS as an upscaler");
    let adv = get_optiscaler_advisory(&g).expect("OptiScaler should provide DX11 interop notice for XeSS");
    assert_eq!(adv.severity, AdvisorySeverity::Info, "Supported DX11 D3D11on12 bridging must have Info severity, not Warning");
    assert!(adv.reasons.iter().any(|r| r.contains("DirectX 11 Interop Notice")));
    assert!(!adv.reasons.iter().any(|r| r.contains("Missing Native Upscaler")));
    assert!(adv.recommendation.contains("bridge DirectX 11 DLSS calls to D3D12"));
}

#[test]
fn test_optiscaler_dx11_bg3_with_native_dlss_gives_info_severity() {
    let mut g = make_test_game("DirectX 11", 64, Some("3.7.10"));
    g.name = "Baldur's Gate 3".to_string();
    g.files.push(GameFileItem {
        rel: "bin\\nvngx_dlss.dll".to_string(),
        version: Some("3.7.10".to_string()),
    });
    assert!(g.has_native_dlss(), "Must detect native DLSS");
    let adv = get_optiscaler_advisory(&g).expect("OptiScaler should provide DX11 interop notice for BG3 DX11");
    assert_eq!(adv.severity, AdvisorySeverity::Info, "BG3 DX11 with native DLSS must be Info severity (supported via Dx11Upscaler=dlss_12)");
    assert!(adv.reasons.iter().any(|r| r.contains("DirectX 11 Interop Notice")));
    assert!(adv.recommendation.contains("bridge DirectX 11 DLSS calls to D3D12"));
}

#[test]
fn test_frame_generation_status_dx11_reports_d3d11on12_when_dlss_present() {
    let mut g = make_test_game("DirectX 11", 64, Some("3.7.10"));
    g.name = "Baldur's Gate 3".to_string();
    g.available_exes.push(dlss_studio::core::scan::GameExeOption {
        name: "bg3.exe".to_string(),
        path: PathBuf::from("C:\\Games\\BG3\\bin\\bg3.exe"),
        rel: "bin\\bg3.exe".to_string(),
        api: "Vulkan".to_string(),
        bitness: 64,
        is_laa: false,
    });
    let (status, is_active) = frame_generation_status(&g);
    assert!(!is_active);
    assert_eq!(status, "Injectable (D3D11on12 DLSS MFG)");
}

#[test]
fn test_frame_generation_status_vulkan_reports_unsupported() {
    let g = make_test_game("Vulkan", 64, Some("3.7.10"));
    let (status, is_active) = frame_generation_status(&g);
    assert!(!is_active);
    assert_eq!(status, "Unsupported (Vulkan API)");
}

#[test]
fn test_optiscaler_dx11_12_with_native_dlss_has_no_advisory() {
    let mut g = make_test_game("DirectX 11/12", 64, Some("2.4.12"));
    g.name = "A Plague Tale: Requiem".to_string();
    g.has_frame_generation = true;

    assert!(get_optiscaler_advisory(&g).is_none(), "Games capable of DirectX 12 must not show D3D11on12 incompatibility warnings");
    assert!(get_native_dlss_advisory(&g).is_none(), "Games capable of DirectX 12 must support Native DLSS");
    assert!(get_mfg_advisory(&g, true).is_none(), "Games with native DLSS-G must support MFG");
}

#[test]
fn test_optiscaler_advisory_not_suppressed_when_feeder_mod_installed() {
    let temp = TempDir::new("anti_suppression");
    std::fs::create_dir_all(temp.join("_DLSS5_Backup")).unwrap();
    
    let manifest = ActiveManifest {
        version: 1,
        date: "now".to_string(),
        route: "feeder".to_string(),
        game: None,
        game_exe: Some("game.exe".to_string()),
        backup_prefix: None,
        replaced: Vec::new(), // vanilla never had DLSS
        added: vec!["nvngx_dlss.dll".to_string(), "dlss5-feed.addon64".to_string()], // mod deployed it!
        added_dirs: Vec::new(),
        mfg_unlock: None,
        mfg_multiplier: None,
        nr_style_enabled: None,
        nr_style: None,
        opti_presr: None,
        opti_passes: None,
    };
    std::fs::write(temp.join("_DLSS5_Backup\\manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();

    let mut g = make_test_game("DirectX 11", 64, Some("310.8.0.0"));
    g.dir = temp.path().to_path_buf();
    
    assert!(!g.has_native_dlss(), "DLSS added by Feeder mod must NOT be counted as native DLSS");
    
    let adv = get_optiscaler_advisory(&g).expect("OptiScaler must NOT suppress warning on Feeder-patched DX11 title");
    assert!(adv.reasons.iter().any(|r| r.contains("Non-DirectX 12 / Missing Native Upscaler")));
    assert!(adv.reasons.iter().any(|r| r.contains("Mod-Deployed DLSS Detected")));
}

#[test]
fn test_mfg_advisory_with_route_warns_on_vulkan() {
    let mut g = make_test_game("Vulkan", 64, Some("3.7.0"));
    g.name = "Baldur's Gate 3 (Vulkan)".to_string();
    let adv = get_mfg_advisory_with_route(&g, true, Some("optiscaler"), None)
        .expect("Vulkan must have MFG advisory");
    assert_eq!(adv.severity, AdvisorySeverity::Warning);
    assert!(adv.reasons.iter().any(|r| r.contains("Vulkan Limitation")));
    assert!(adv.recommendation.contains("Vulkan"));
}

#[test]
fn test_mfg_advisory_with_route_warns_on_feeder_route() {
    let mut g = make_test_game("DirectX 11", 64, Some("3.7.0"));
    g.name = "Baldur's Gate 3 (DX11)".to_string();
    let adv = get_mfg_advisory_with_route(&g, true, Some("reshade"), Some("feeder"))
        .expect("Feeder route must have MFG advisory");
    assert_eq!(adv.severity, AdvisorySeverity::Warning);
    assert!(adv.reasons.iter().any(|r| r.contains("DLSS 5 Feeder Limitation")));
    assert!(adv.recommendation.contains("OptiScaler DLSS-NR"));
}

#[test]
fn test_mfg_advisory_with_route_clean_on_dx11_optiscaler() {
    let mut g = make_test_game("DirectX 11", 64, Some("3.7.0"));
    g.name = "Baldur's Gate 3 (DX11)".to_string();
    let adv = get_mfg_advisory_with_route(&g, true, Some("optiscaler"), None).expect("Must return Info notice for DX11 D3D11on12");
    assert_eq!(adv.severity, AdvisorySeverity::Info);
    assert!(adv.reasons.iter().any(|r| r.contains("DirectX 11 Interop Notice")));
}

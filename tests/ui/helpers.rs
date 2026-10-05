use dlss_studio::ui::helpers::*;
use dlss_studio::core::scan::GameEntry;
use dlss_studio::core::journal::HistoryRow;

#[test]
fn test_clean_display_title_formats() {
    assert_eq!(clean_display_title("Cyberpunk_2077"), "Cyberpunk 2077");
    assert_eq!(clean_display_title("The-Witcher-3"), "The Witcher 3");
    assert_eq!(clean_display_title("Package.Name.GameTitle_1.0_x64"), "Game Title");
    assert_eq!(clean_display_title("Microsoft.FlightSimulator_1.37.19.0_x64__8wekyb3d8bbwe"), "Flight Simulator");
    assert_eq!(clean_display_title("SimpleTitle"), "SimpleTitle");
}

#[test]
fn test_resolve_game_title_logic() {
    let games = vec![
        GameEntry {
            name: "Baldur's Gate 3".to_string(),
            dir: std::path::PathBuf::from("C:\\Games\\Baldurs Gate 3"),
            ..Default::default()
        }
    ];

    let row1 = HistoryRow {
        game_name: Some("Direct Name".to_string()),
        dir: "C:\\Games\\Unknown".to_string(),
        ..Default::default()
    };
    assert_eq!(resolve_game_title(&row1, &games), "Direct Name");

    let row2 = HistoryRow {
        game_name: None,
        dir: "C:\\Games\\Baldurs Gate 3".to_string(),
        ..Default::default()
    };
    assert_eq!(resolve_game_title(&row2, &games), "Baldur's Gate 3");

    let row3 = HistoryRow {
        game_name: None,
        dir: "C:\\Games\\Starfield\\Content".to_string(),
        ..Default::default()
    };
    assert_eq!(resolve_game_title(&row3, &[]), "Starfield");

    let row4 = HistoryRow {
        game_name: None,
        dir: "C:\\Games\\Cyberpunk\\Binaries\\Win64".to_string(),
        ..Default::default()
    };
    assert_eq!(resolve_game_title(&row4, &[]), "Cyberpunk");
}

#[test]
fn test_resolve_module_meta() {
    let (key, vendor, class, tag) = resolve_module_meta("nvngx_dlss.dll");
    assert_eq!(key, "module_dlss_sr");
    assert_eq!(vendor, "NVIDIA");
    assert_eq!(class, "vendor-nvidia");
    assert_eq!(tag, "DLSS");

    let (key2, vendor2, _, _) = resolve_module_meta("bin/x64/nvngx_dlssg.dll");
    assert_eq!(key2, "module_dlss_fg");
    assert_eq!(vendor2, "NVIDIA");

    let (key3, _, _, _) = resolve_module_meta("sl.reflex.dll");
    assert_eq!(key3, "module_sl_reflex");

    let (rr_key, _, _, rr_tag) = resolve_module_meta("nvngx_dlssd.dll");
    assert_eq!(rr_key, "module_dlss_rr");
    assert_eq!(rr_tag, "DLSS-RR");

    let (fsr_key, fsr_vendor, _, _) = resolve_module_meta("amd_fidelityfx_framegeneration_dx12.dll");
    assert_eq!(fsr_key, "module_fsr_fg");
    assert_eq!(fsr_vendor, "AMD");

    let (sl_dlss, _, _, _) = resolve_module_meta("sl.dlss.dll");
    assert_eq!(sl_dlss, "module_sl_dlss");

    let (sl_fg, _, _, _) = resolve_module_meta("sl.dlss_g.dll");
    assert_eq!(sl_fg, "module_sl_fg");

    let (sl_core, _, _, _) = resolve_module_meta("sl.common.dll");
    assert_eq!(sl_core, "module_sl_core");

    let (opti_key, opti_vendor, _, _) = resolve_module_meta("optiscaler/dxgi.dll");
    assert_eq!(opti_key, "module_optiscaler");
    assert_eq!(opti_vendor, "OptiScaler");

    let (key4, _, _, _) = resolve_module_meta("custom_mod.dll");
    assert_eq!(key4, "module_generic_dll");
}

#[test]
fn test_copy_to_clipboard_safe_call() {
    copy_to_clipboard("DLSS Studio Unit Test Clipboard");
}

#[test]
fn test_app_status_formatting_all_variants() {
    let lang = "en";
    let statuses = vec![
        AppStatus::Ready,
        AppStatus::DownloadingComponents,
        AppStatus::DownloadFailed("Network timeout".to_string()),
        AppStatus::ScanningLibrary,
        AppStatus::ScanningFolder("C:\\Games".to_string()),
        AppStatus::ScanningLaunchers,
        AppStatus::FoundGames(12),
        AppStatus::NoGamesFound("C:\\Empty".to_string()),
        AppStatus::AddedGame("Cyberpunk 2077".to_string()),
    ];

    for s in statuses {
        let text = format_status(lang, &s);
        assert!(!text.is_empty(), "Status {:?} formatted string should not be empty", s);
    }
}

#[test]
fn test_brand_badge_constant() {
    assert!(!BRAND_BADGE_WEBP.is_empty());
}



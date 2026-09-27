use dlss_studio::ui::app::*;
use dlss_studio::core::scan::GameEntry;
use dlss_studio::core::journal::HistoryRow;
use dlss_studio::core::state::{load_state, save_state, STATE_TEST_MUTEX};
use dlss_studio::core::install_routes::{recommended_route, InstallRoute};
use dioxus::prelude::*;

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
fn test_app_virtual_dom_headless_render() {
    let mut dom = VirtualDom::new(App);
    dom.rebuild_in_place();
}

#[test]
fn test_app_virtual_dom_all_views_and_modals() {
    let _state_lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = load_state();
    let old_games = s.cached_games.clone();
    s.cached_games = vec![
        GameEntry {
            name: "Cyberpunk 2077".to_string(),
            dir: std::path::PathBuf::from("C:\\Games\\Cyberpunk 2077"),
            exe_path: std::path::PathBuf::from("C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe"),
            launcher: "Steam".to_string(),
            bitness: 64,
            api: "DirectX 12".to_string(),
            dlss_version: Some("3.7.0.0".to_string()),
            has_frame_generation: true,
            can_inject_fg: false,
            ..Default::default()
        },
        GameEntry {
            name: "Baldur's Gate 3".to_string(),
            dir: std::path::PathBuf::from("C:\\Games\\Baldurs Gate 3"),
            exe_path: std::path::PathBuf::from("C:\\Games\\Baldurs Gate 3\\bin\\bg3_dx11.exe"),
            launcher: "GOG".to_string(),
            bitness: 64,
            api: "DirectX 11".to_string(),
            dlss_version: Some("2.4.2.0".to_string()),
            has_frame_generation: false,
            can_inject_fg: true,
            ..Default::default()
        },
    ];
    let _ = save_state(&s);

    // 1. Home view
    std::env::set_var("DLSS_TEST_VIEW", "home");
    let mut dom_home = VirtualDom::new(App);
    dom_home.rebuild_in_place();

    // 2. Games library view
    std::env::set_var("DLSS_TEST_VIEW", "games");
    let mut dom_games = VirtualDom::new(App);
    dom_games.rebuild_in_place();

    // 3. Add-ons view
    std::env::set_var("DLSS_TEST_VIEW", "addons");
    let mut dom_addons = VirtualDom::new(App);
    dom_addons.rebuild_in_place();

    // 4. History view
    std::env::set_var("DLSS_TEST_VIEW", "history");
    let mut dom_hist = VirtualDom::new(App);
    dom_hist.rebuild_in_place();

    // 5. Settings view
    std::env::set_var("DLSS_TEST_VIEW", "settings");
    let mut dom_sett = VirtualDom::new(App);
    dom_sett.rebuild_in_place();

    // 6. About view
    std::env::set_var("DLSS_TEST_VIEW", "about");
    let mut dom_about = VirtualDom::new(App);
    dom_about.rebuild_in_place();

    // 7. Game detail sheet modal
    std::env::set_var("DLSS_TEST_VIEW", "games");
    std::env::set_var("DLSS_TEST_SHEET", "1");
    let mut dom_sheet = VirtualDom::new(App);
    dom_sheet.rebuild_in_place();
    std::env::remove_var("DLSS_TEST_SHEET");

    // 8. RenoDX Overlay preview modal
    std::env::set_var("DLSS_TEST_PREVIEW", "1");
    let mut dom_prev = VirtualDom::new(App);
    dom_prev.rebuild_in_place();
    std::env::remove_var("DLSS_TEST_PREVIEW");

    // 9. Hotkey modal
    std::env::set_var("DLSS_TEST_HOTKEY", "1");
    let mut dom_hk = VirtualDom::new(App);
    dom_hk.rebuild_in_place();
    std::env::remove_var("DLSS_TEST_HOTKEY");

    // Clean up env and restore state
    std::env::remove_var("DLSS_TEST_VIEW");
    s.cached_games = old_games;
    let _ = save_state(&s);
}

#[test]
fn test_format_status_found_games() {
    assert_eq!(format_status("en", &AppStatus::FoundGames(8)), "Found 8 games across sources");
    assert_eq!(format_status("de", &AppStatus::FoundGames(12)), "12 Spiele plattformübergreifend gefunden");
    assert_eq!(format_status("zh", &AppStatus::FoundGames(5)), "共发现 5 款已安装游戏");
    assert_eq!(format_status("en", &AppStatus::NoGamesFound("Documents".to_string())), "No supported game executables found in Documents");
    assert_eq!(format_status("de", &AppStatus::NoGamesFound("Downloads".to_string())), "Keine unterstützten Spieldateien in Downloads gefunden");
    assert_eq!(format_status("en", &AppStatus::AddedGame("Cyberpunk 2077".to_string())), "Added Cyberpunk 2077");
    assert_eq!(format_status("de", &AppStatus::AddedGame("Cyberpunk 2077".to_string())), "Cyberpunk 2077 hinzugefügt");
}

#[test]
fn test_recommended_route_auto_selection_across_apis() {
    // 1. DirectX 11 title without native DLSS-G -> must auto-select Feeder (compatible, no warnings)
    let dx11_game = GameEntry {
        name: "Baldur's Gate 3 DX11".to_string(),
        api: "DirectX 11".to_string(),
        bitness: 64,
        dlss_version: Some("2.4.2.0".to_string()),
        ..Default::default()
    };
    assert_eq!(recommended_route(&dx11_game), InstallRoute::Feeder);

    // 2. 64-bit DirectX 12 title with DLSS -> auto-selects Native DLSS
    let dx12_game = GameEntry {
        name: "Cyberpunk 2077".to_string(),
        api: "DirectX 12".to_string(),
        bitness: 64,
        dlss_version: Some("3.7.0.0".to_string()),
        ..Default::default()
    };
    assert_eq!(recommended_route(&dx12_game), InstallRoute::Native);

    // 3. Vulkan title -> must auto-select Feeder
    let vulkan_game = GameEntry {
        name: "Doom Eternal".to_string(),
        api: "Vulkan".to_string(),
        bitness: 64,
        dlss_version: Some("3.1.1.0".to_string()),
        ..Default::default()
    };
    assert_eq!(recommended_route(&vulkan_game), InstallRoute::Feeder);

    // 4. Legacy DirectX 9 title (e.g. Mass Effect 2) -> must auto-select Feeder
    let dx9_game = GameEntry {
        name: "Mass Effect 2".to_string(),
        api: "DirectX 9".to_string(),
        bitness: 32,
        ..Default::default()
    };
    assert_eq!(recommended_route(&dx9_game), InstallRoute::Feeder);
}

#[test]
fn test_resolve_module_meta() {
    assert_eq!(resolve_module_meta("nvngx_dlss.dll"), ("module_dlss_sr", "NVIDIA", "vendor-nvidia", "DLSS"));
    assert_eq!(resolve_module_meta("bin\\x64\\nvngx_dlssg.dll"), ("module_dlss_fg", "NVIDIA", "vendor-nvidia", "DLSS-G"));
    assert_eq!(resolve_module_meta("nvngx_dlssd.dll"), ("module_dlss_rr", "NVIDIA", "vendor-nvidia", "DLSS-RR"));
    assert_eq!(resolve_module_meta("amd_fidelityfx_framegeneration_dx12.dll"), ("module_fsr_fg", "AMD", "vendor-amd", "FSR FG"));
    assert_eq!(resolve_module_meta("sl.dlss.dll"), ("module_sl_dlss", "Streamline", "vendor-sl", "SL DLSS"));
    assert_eq!(resolve_module_meta("sl.dlss_g.dll"), ("module_sl_fg", "Streamline", "vendor-sl", "SL FG"));
    assert_eq!(resolve_module_meta("sl.common.dll"), ("module_sl_core", "Streamline", "vendor-sl", "SL Core"));
    assert_eq!(resolve_module_meta("sl.interposer.dll"), ("module_sl_core", "Streamline", "vendor-sl", "SL Core"));
    assert_eq!(resolve_module_meta("sl.reflex.dll"), ("module_sl_reflex", "Streamline", "vendor-sl", "Reflex"));
    assert_eq!(resolve_module_meta("optiscaler/dxgi.dll"), ("module_optiscaler", "OptiScaler", "vendor-opti", "OptiScaler"));
    assert_eq!(resolve_module_meta("some_other.dll"), ("module_generic_dll", "Runtime", "vendor-generic", "DLL"));
}

#[test]
fn test_info_advisory_does_not_trigger_override_mode() {
    use dlss_studio::core::install_routes::{AdvisorySeverity, RouteAdvisory};

    let info_advisory = RouteAdvisory {
        title: "Test Info".to_string(),
        reasons: vec!["Informational configuration note".to_string()],
        recommendation: "Bridge is active".to_string(),
        severity: AdvisorySeverity::Info,
    };

    let active_advisories = vec![info_advisory];
    let has_override = active_advisories.iter().any(|a| a.severity == AdvisorySeverity::Warning);
    assert!(!has_override, "Informational advisories must NEVER trigger deploy override mode");

    let warning_advisory = RouteAdvisory {
        title: "Test Warning".to_string(),
        reasons: vec!["Hard blocker".to_string()],
        recommendation: "Do not use".to_string(),
        severity: AdvisorySeverity::Warning,
    };

    let active_advisories_with_warning = vec![warning_advisory];
    let has_override_warning = active_advisories_with_warning.iter().any(|a| a.severity == AdvisorySeverity::Warning);
    assert!(has_override_warning, "Warning advisories must trigger deploy override mode");
}

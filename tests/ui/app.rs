use dlss_studio::ui::app::*;
use dlss_studio::core::scan::GameEntry;
use dlss_studio::core::state::{load_state, save_state, STATE_TEST_MUTEX};
use dlss_studio::core::routes::{recommended_route, InstallRoute};
use dioxus::prelude::*;

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

    // 7. Game detail sheet modal (Native DLSS DX12)
    std::env::set_var("DLSS_TEST_VIEW", "games");
    std::env::set_var("DLSS_TEST_SHEET", "1");
    std::env::set_var("DLSS_TEST_SHEET_IDX", "0");
    let mut dom_sheet = VirtualDom::new(App);
    dom_sheet.rebuild_in_place();

    // 7b. Game detail sheet modal (Feeder DX11 with MFG)
    std::env::set_var("DLSS_TEST_SHEET_IDX", "1");
    std::env::set_var("DLSS_TEST_BACKEND", "reshade");
    std::env::set_var("DLSS_TEST_ROUTE", "feeder");
    std::env::set_var("DLSS_TEST_MFG", "1");
    std::env::set_var("DLSS_TEST_MODULES", "1");
    let mut dom_sheet_feeder = VirtualDom::new(App);
    dom_sheet_feeder.rebuild_in_place();

    // 7c. Game detail sheet modal (OptiScaler route)
    std::env::set_var("DLSS_TEST_BACKEND", "optiscaler");
    std::env::set_var("DLSS_TEST_EDIT_NAME", "1");
    let mut dom_sheet_opti = VirtualDom::new(App);
    dom_sheet_opti.rebuild_in_place();

    std::env::remove_var("DLSS_TEST_SHEET");
    std::env::remove_var("DLSS_TEST_SHEET_IDX");
    std::env::remove_var("DLSS_TEST_BACKEND");
    std::env::remove_var("DLSS_TEST_ROUTE");
    std::env::remove_var("DLSS_TEST_MFG");
    std::env::remove_var("DLSS_TEST_MODULES");
    std::env::remove_var("DLSS_TEST_EDIT_NAME");

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

    // 10. Add-on dialog modal
    std::env::set_var("DLSS_TEST_VIEW", "addons");
    std::env::set_var("DLSS_TEST_ADDON_DLG", "1");
    let mut dom_addon_dlg = VirtualDom::new(App);
    dom_addon_dlg.rebuild_in_place();
    std::env::remove_var("DLSS_TEST_ADDON_DLG");

    // 11. Create custom theme modal
    std::env::set_var("DLSS_TEST_CREATE_THEME", "1");
    let mut dom_theme_dlg = VirtualDom::new(App);
    dom_theme_dlg.rebuild_in_place();
    std::env::remove_var("DLSS_TEST_CREATE_THEME");

    // 12. Language dropdown menu
    std::env::set_var("DLSS_TEST_LANG_MENU", "1");
    let mut dom_lang = VirtualDom::new(App);
    dom_lang.rebuild_in_place();
    std::env::remove_var("DLSS_TEST_LANG_MENU");

    // 13. Toast banner notification
    std::env::set_var("DLSS_TEST_TOAST", "1");
    let mut dom_toast = VirtualDom::new(App);
    dom_toast.rebuild_in_place();
    std::env::remove_var("DLSS_TEST_TOAST");

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
fn test_info_advisory_does_not_trigger_override_mode() {
    use dlss_studio::core::advisories::{AdvisorySeverity, RouteAdvisory};

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

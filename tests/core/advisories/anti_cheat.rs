use dlss_studio::core::advisories::anti_cheat::*;
use dlss_studio::core::advisories::AdvisorySeverity;
use dlss_studio::core::scan::GameEntry;
use std::path::PathBuf;

fn make_test_game() -> GameEntry {
    GameEntry {
        name: "AntiCheat Test Game".to_string(),
        dir: PathBuf::from("C:\\Games\\Test"),
        exe_path: PathBuf::from("C:\\Games\\Test\\game.exe"),
        exe_rel: "game.exe".to_string(),
        bitness: 64,
        api: "DirectX 12".to_string(),
        dlss_version: Some("3.7.0".to_string()),
        has_anti_cheat: false,
        ..Default::default()
    }
}

#[test]
fn test_anti_cheat_advisory_detection() {
    let mut g = make_test_game();
    assert!(get_anti_cheat_advisory(&g).is_none());

    g.has_anti_cheat = true;
    let adv = get_anti_cheat_advisory(&g).expect("Must return anti-cheat advisory");
    assert_eq!(adv.severity, AdvisorySeverity::Warning);
    assert!(adv.title.contains("Anti-Cheat Detected"));
    assert!(adv.reasons.iter().any(|r| r.contains("anti-cheat middleware")));
}

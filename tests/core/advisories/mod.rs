pub mod anti_cheat;
pub mod mfg;

use dlss_studio::core::advisories::*;
use dlss_studio::core::scan::GameEntry;
use std::path::PathBuf;

#[test]
fn test_collect_advisories_clean_game() {
    let game = GameEntry {
        name: "Advisory Test Game".to_string(),
        dir: PathBuf::from("C:\\Games\\Test"),
        exe_path: PathBuf::from("C:\\Games\\Test\\game.exe"),
        exe_rel: "game.exe".to_string(),
        bitness: 64,
        api: "DirectX 12".to_string(),
        dlss_version: Some("3.7.0".to_string()),
        has_frame_generation: true,
        has_anti_cheat: false,
        ..Default::default()
    };

    let advisories = collect_advisories(&game, "optiscaler", "optiscaler", false, true);
    assert!(advisories.is_empty(), "Ideal DX12 game with DLSS should have zero advisories for OptiScaler");
}

#[test]
fn test_get_optiscaler_advisory_dx11_without_upscaler() {
    let game = GameEntry {
        name: "DX11 Game".to_string(),
        dir: PathBuf::from("C:\\Games\\Test"),
        exe_path: PathBuf::from("C:\\Games\\Test\\game.exe"),
        exe_rel: "game.exe".to_string(),
        bitness: 64,
        api: "DirectX 11".to_string(),
        ..Default::default()
    };

    let adv = get_optiscaler_advisory(&game).expect("Must return advisory for DX11 without native upscaler");
    assert_eq!(adv.severity, AdvisorySeverity::Warning);
}

use dlss_studio::core::advisories::mfg::*;
use dlss_studio::core::scan::GameEntry;
use std::path::PathBuf;

fn make_test_game(api: &str, bitness: u32, dlss: Option<&str>) -> GameEntry {
    GameEntry {
        name: "MFG Advisory Test Game".to_string(),
        dir: PathBuf::from("C:\\Games\\Test"),
        exe_path: PathBuf::from("C:\\Games\\Test\\game.exe"),
        exe_rel: "game.exe".to_string(),
        bitness,
        api: api.to_string(),
        dlss_version: dlss.map(|s| s.to_string()),
        has_frame_generation: true,
        ..Default::default()
    }
}

#[test]
fn test_mfg_hardware_check_advisories() {
    let g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    let adv_non_ada = get_mfg_advisory(&g, false).expect("Non-RTX 40 GPU must trigger advisory");
    assert!(adv_non_ada.reasons.iter().any(|r| r.contains("Ada Lovelace") || r.contains("RTX 40-Series")));
}

#[test]
fn test_mfg_ada_hardware_advisory() {
    let g = make_test_game("DirectX 12", 64, Some("3.7.0"));
    let adv_ada = get_mfg_advisory(&g, true);
    // On Ada Lovelace with DX12 and native DLSS-G, no advisory should be present
    assert!(adv_ada.is_none());
}

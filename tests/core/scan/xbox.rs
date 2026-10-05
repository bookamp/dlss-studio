use dlss_studio::core::scan::xbox::*;
use dlss_studio::core::scan::scan_game_directory;
use std::path::Path;

#[test]
fn test_discover_xbox_runs_safely() {
    let games = discover_xbox();
    for game in games {
        assert_eq!(game.launcher, "Xbox");
        assert!(!game.name.is_empty());
    }
}

#[test]
fn test_control_pcgp_live_detection() {
    let live_path = Path::new(r"D:\WindowsApps\505GAMESS.P.A.ControlPCGP_1.0.6.0_x64__tefn33qh9azfc");
    if live_path.is_dir() {
        let game = scan_game_directory(live_path).expect("Live Control PCGP must scan");
        assert_eq!(game.api, "DirectX 12", "Live Control PCGP must detect as DirectX 12");
        let routes = dlss_studio::core::routes::routes_for(&game);
        assert!(routes.contains(&dlss_studio::core::routes::InstallRoute::Native), "Live Control PCGP must support Native DLSS");
        assert!(routes.contains(&dlss_studio::core::routes::InstallRoute::OptiScaler), "Live Control PCGP must support OptiScaler");
        assert!(routes.contains(&dlss_studio::core::routes::InstallRoute::Feeder), "Live Control PCGP must support Feeder");
    }
}

#[test]
fn test_xbox_executables_config_parser() {
    let temp = crate::common::TempDir::new("xbox_cfg");
    let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
    <ExecutableList>
        <Executable Name="MainGame.exe" TargetDeviceFamily="PC" Architecture="x64"/>
    </ExecutableList>
    <ProcessorArchitecture>x64</ProcessorArchitecture>
</Game>"#;
    std::fs::write(temp.join("MicrosoftGame.config"), xml).unwrap();
    std::fs::write(temp.join("MainGame.exe"), b"MZ dummy PE").unwrap();

    let exes = xbox_executables(temp.path());
    assert_eq!(exes.len(), 1);
    assert_eq!(exes[0].name, "MainGame.exe");
    assert_eq!(exes[0].bitness, 64);
}



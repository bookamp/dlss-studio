pub mod heuristics;
pub mod steam;
pub mod epic;
pub mod gog;
pub mod xbox;

use dlss_studio::core::scan::*;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_scan_synthetic_game_directory() {
    let temp_dir = std::env::temp_dir().join(format!("dlss_scan_synthetic_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    let bin_dir = temp_dir.join("bin").join("x64");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("CyberGame.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();

    fs::write(bin_dir.join("D3D12Core.dll"), b"core").unwrap();
    fs::write(bin_dir.join("nvngx_dlss.dll"), b"dlss").unwrap();
    fs::write(bin_dir.join("nvngx_dlssg.dll"), b"framegen").unwrap();

    let g = scan_game_directory(&temp_dir).expect("Synthetic game must be scanned");
    assert_eq!(g.api, "DirectX 12");
    assert!(g.has_frame_generation, "Frame generation must be detected from nvngx_dlssg.dll");
    assert_eq!(g.exe_rel, "bin\\x64\\CyberGame.exe");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scan_dlss2_without_native_fg_marks_can_inject_fg() {
    let temp_dir = std::env::temp_dir().join(format!("dlss_scan_dlss2_only_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    let bin_dir = temp_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("bg3.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..119].copy_from_slice(b"vkCreateInstance\x00\x00\x00");
    fs::write(&exe_path, &exe_bytes).unwrap();

    fs::write(bin_dir.join("vulkan-1.dll"), b"vulkan").unwrap();
    fs::write(bin_dir.join("nvngx_dlss.dll"), b"dlss").unwrap();

    let g = scan_game_directory(&temp_dir).expect("Synthetic Vulkan game must be scanned");
    assert_eq!(g.api, "Vulkan");
    assert!(!g.has_frame_generation, "Native frame generation is false");
    assert!(!g.can_inject_fg, "can_inject_fg must be false for Vulkan game");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scan_control_dx12_marks_can_inject_fg() {
    let temp_dir = std::env::temp_dir().join(format!("dlss_scan_dx12_fg_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let bin_dir = temp_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = bin_dir.join("Control_DX12.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..118].copy_from_slice(b"D3D12CreateDevice\x00");
    fs::write(&exe_path, &exe_bytes).unwrap();

    fs::write(bin_dir.join("d3d12.dll"), b"dx12").unwrap();
    fs::write(bin_dir.join("nvngx_dlss.dll"), b"dlss").unwrap();

    let g = scan_game_directory(&temp_dir).expect("Synthetic DX12 game must be scanned");
    assert_eq!(g.api, "DirectX 12");
    assert!(!g.has_frame_generation, "Native frame generation is false");
    assert!(g.can_inject_fg, "can_inject_fg must be true for 64-bit DirectX 12 game with DLSS 2");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scan_ignores_optiscaler_subdirectories_for_fg() {
    let temp_dir = std::env::temp_dir().join(format!("dlss_scan_opti_ignore_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    let bin_dir = temp_dir.join("bin");
    let opti_streamline = bin_dir.join("OptiScaler").join("streamline");
    fs::create_dir_all(&opti_streamline).unwrap();

    let exe_path = bin_dir.join("game.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..119].copy_from_slice(b"D3D11CreateDevice\x00\x00");
    fs::write(&exe_path, &exe_bytes).unwrap();

    fs::write(bin_dir.join("d3d11.dll"), b"dx11").unwrap();
    fs::write(bin_dir.join("nvngx_dlss.dll"), b"dlss").unwrap();
    fs::write(opti_streamline.join("nvngx_dlssg.dll"), b"bundled_dlssg").unwrap();
    fs::write(opti_streamline.join("sl.dlss_g.dll"), b"bundled_sl_dlssg").unwrap();

    let g = scan_game_directory(&temp_dir).expect("Synthetic DX11 game must be scanned");
    assert_eq!(g.api, "DirectX 11");
    assert!(!g.has_frame_generation, "Bundled OptiScaler streamline files must not trigger native has_frame_generation");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_manifest_preserves_target_exe_and_patch_state() {
    let temp_dir = std::env::temp_dir().join(format!("test_manifest_preserves_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let bin_dir = temp_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    fs::write(bin_dir.join("game.exe"), b"MZ dummy 64-bit exe").unwrap();
    fs::write(bin_dir.join("game_dx11.exe"), b"MZ dummy 64-bit exe").unwrap();
    fs::write(bin_dir.join("nvngx_dlss.dll"), b"MZ dlss").unwrap();

    let bdir = temp_dir.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();

    let manifest = dlss_studio::core::journal::ActiveManifest {
        version: 1,
        date: "2026-09-18 12:00:00".to_string(),
        route: "native".to_string(),
        game: Some(dlss_studio::core::journal::ManifestGame {
            dir: Some(temp_dir.to_string_lossy().to_string()),
            exe: Some("bin\\game_dx11.exe".to_string()),
            api: Some("dxgi".to_string()),
            bitness: Some(64),
            api_label: Some("DirectX 11".to_string()),
        }),
        game_exe: Some("bin\\game_dx11.exe".to_string()),
        backup_prefix: Some("originals/123".to_string()),
        replaced: Vec::new(),
        added: vec!["bin\\dxgi.dll".to_string(), "bin\\renodx-dlss5.addon64".to_string()],
        added_dirs: Vec::new(),
        mfg_unlock: Some(true),
        mfg_multiplier: Some(4),
        nr_style_enabled: Some(true),
        nr_style: Some(0),
        opti_presr: Some(false),
        opti_passes: Some(1),
    };
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();

    let scanned = scan_game_directory(&temp_dir).expect("Game directory must scan");
    assert_eq!(scanned.exe_rel, "bin\\game_dx11.exe");
    assert_eq!(scanned.installed_route, Some("native".to_string()));
    assert!(scanned.mfg_unlock_installed);
    assert_eq!(scanned.mfg_multiplier, 4);
    assert_eq!(scanned.nr_style, 0);
    assert!(scanned.nr_style_enabled);
    assert!(scanned.reshade_installed);
    assert!(scanned.addon_installed);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_scan_infers_route_from_disk_when_manifest_missing() {
    let temp_dir = std::env::temp_dir().join(format!("test_disk_infer_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let bin_dir = temp_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let exe_path = bin_dir.join("game.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..117].copy_from_slice(b"D3D12CreateDevice");
    fs::write(&exe_path, &exe_bytes).unwrap();

    fs::write(bin_dir.join("renodx-dlss5.addon64"), b"DUMMY_ADDON").unwrap();
    let scanned = scan_game_directory(&temp_dir).expect("Scan must succeed");
    assert_eq!(scanned.installed_route, Some("native".to_string()));

    fs::write(bin_dir.join("dlss5-feed.addon64"), b"DUMMY_FEEDER").unwrap();
    let scanned2 = scan_game_directory(&temp_dir).expect("Scan must succeed");
    assert_eq!(scanned2.installed_route, Some("feeder".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_game_entry_has_anti_cheat_serde_backwards_compatibility() {
    let old_json = r#"{
        "name": "Classic Game",
        "dir": "C:\\Games\\Classic",
        "exe_path": "C:\\Games\\Classic\\game.exe",
        "exe_rel": "game.exe",
        "bitness": 64,
        "api": "DirectX 11",
        "has_frame_generation": false,
        "optiscaler_installed": false,
        "optiscaler_presr": false,
        "optiscaler_passes": 1,
        "mfg_unlock_installed": false,
        "has_backup": false,
        "launcher": "Steam",
        "reshade_installed": false,
        "reshade_addon_support": false,
        "addon_installed": false,
        "files": []
    }"#;

    let entry: GameEntry = serde_json::from_str(old_json).expect("deserialize old json");
    assert!(!entry.has_anti_cheat);

    let mut modern = entry.clone();
    modern.has_anti_cheat = true;
    let serialized = serde_json::to_string(&modern).expect("serialize modern entry");
    let restored: GameEntry = serde_json::from_str(&serialized).expect("deserialize modern entry");
    assert!(restored.has_anti_cheat);
}

#[test]
fn test_scan_game_directory_detects_anti_cheat() {
    let temp_dir = std::env::temp_dir().join(format!("test_ac_scan_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    fs::write(temp_dir.join("game.exe"), b"DUMMY_EXE_CONTENT").unwrap();
    fs::write(temp_dir.join("EasyAntiCheat_x64.dll"), b"EAC").unwrap();

    let scanned = scan_game_directory(&temp_dir);
    assert!(scanned.is_some());
    let game = scanned.unwrap();
    assert!(game.has_anti_cheat);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_game_entry_available_exes_cached_reuse() {
    let mut entry = GameEntry::default();
    entry.available_exes = vec![
        GameExeOption {
            name: "ACOdyssey.exe".to_string(),
            path: PathBuf::from(r"E:\Games\Assassin's Creed Odyssey\ACOdyssey.exe"),
            rel: "ACOdyssey.exe".to_string(),
            api: "DirectX 11".to_string(),
            bitness: 64,
            is_laa: true,
        },
        GameExeOption {
            name: "ACOdyssey_plus.exe".to_string(),
            path: PathBuf::from(r"E:\Games\Assassin's Creed Odyssey\ACOdyssey_plus.exe"),
            rel: "ACOdyssey_plus.exe".to_string(),
            api: "DirectX 11".to_string(),
            bitness: 64,
            is_laa: true,
        },
    ];

    assert_eq!(entry.available_exes.len(), 2);
    assert_eq!(entry.available_exes[0].name, "ACOdyssey.exe");
    assert_eq!(entry.available_exes[1].name, "ACOdyssey_plus.exe");
}

#[test]
fn test_mod_added_dlssg_does_not_flag_native_frame_generation() {
    let temp_dir = std::env::temp_dir().join(format!("test_mod_dlssg_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    let dx12_exe = temp_dir.join("game.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..118].copy_from_slice(b"D3D12CreateDevice\x00");
    fs::write(&dx12_exe, &exe_bytes).unwrap();

    fs::write(temp_dir.join("nvngx_dlss.dll"), b"fake_dlss").unwrap();
    fs::write(temp_dir.join("nvngx_dlssg.dll"), b"fake_dlssg").unwrap();

    let backup_dir = temp_dir.join("_DLSS5_Backup");
    fs::create_dir_all(&backup_dir).unwrap();
    let manifest = dlss_studio::core::journal::ActiveManifest {
        version: 1,
        date: "now".to_string(),
        route: "optiscaler".to_string(),
        game: None,
        game_exe: Some("game.exe".to_string()),
        backup_prefix: None,
        replaced: Vec::new(),
        added: vec!["nvngx_dlssg.dll".to_string()],
        added_dirs: Vec::new(),
        mfg_unlock: Some(true),
        mfg_multiplier: Some(4),
        nr_style_enabled: None,
        nr_style: None,
        opti_presr: None,
        opti_passes: None,
    };
    fs::write(backup_dir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();

    let scanned = scan_game_directory(&temp_dir).expect("Game must be scanned");
    assert!(!scanned.has_frame_generation);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_game_entry_route_display_name_and_patch_state() {
    let mut g = GameEntry::default();
    assert_eq!(g.route_display_name(), "Vanilla");
    assert!(!g.is_dlss5_patched());

    g.installed_route = Some("feeder".to_string());
    assert_eq!(g.route_display_name(), "Feeder · Neural Rendering");
    assert!(g.is_dlss5_patched());

    g.installed_route = Some("native".to_string());
    assert_eq!(g.route_display_name(), "Native D3D12");

    g.installed_route = Some("optiscaler".to_string());
    assert_eq!(g.route_display_name(), "OptiScaler");

    g.installed_route = None;
    g.optiscaler_installed = true;
    assert_eq!(g.route_display_name(), "OptiScaler");
    assert!(g.is_dlss5_patched());

    g.optiscaler_installed = false;
    g.reshade_installed = true;
    assert_eq!(g.route_display_name(), "ReShade");
    assert!(g.is_dlss5_patched());
}

#[test]
fn test_game_entry_has_native_upscaler_checks() {
    let mut g = GameEntry::default();
    assert!(!g.has_native_upscaler());

    // With DLSS version
    g.dlss_version = Some("3.7.10".to_string());
    assert!(g.has_native_upscaler());
    g.dlss_version = None;

    // With XeSS
    g.files.push(GameFileItem { rel: "bin\\libxess.dll".to_string(), version: None });
    assert!(g.has_native_upscaler());

    // With FSR2
    g.files.clear();
    g.files.push(GameFileItem { rel: "ffx_fsr2_api_dx12_x64.dll".to_string(), version: None });
    assert!(g.has_native_upscaler());

    // With Streamline Interposer
    g.files.clear();
    g.files.push(GameFileItem { rel: "sl.interposer.dll".to_string(), version: None });
    assert!(g.has_native_upscaler());
}

#[test]
fn test_is_inside_path_containment() {
    use std::path::Path;
    assert!(is_inside(Path::new("C:\\Games\\Steam\\game.exe"), Path::new("C:\\Games\\Steam")));
    assert!(is_inside(Path::new("C:/Games/Steam/bin/game.exe"), Path::new("C:\\Games\\Steam")));
    assert!(is_inside(Path::new("C:\\Games\\Steam"), Path::new("C:\\Games\\Steam")));
    assert!(!is_inside(Path::new("C:\\Other\\game.exe"), Path::new("C:\\Games\\Steam")));
}

#[test]
fn test_dedupe_games_prefers_specific_launcher_over_generic() {
    let mut g1 = GameEntry::default();
    g1.name = "Game A".to_string();
    g1.dir = PathBuf::from("C:\\Games\\GameA");
    g1.launcher = "My folders".to_string();

    let mut g2 = GameEntry::default();
    g2.name = "Game A".to_string();
    g2.dir = PathBuf::from("C:\\Games\\GameA");
    g2.launcher = "Steam".to_string();

    let deduped = dedupe_games(vec![g1, g2]);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].launcher, "Steam");
}

#[test]
fn test_scan_library_root_with_multiple_game_folders() {
    let temp_dir = std::env::temp_dir().join(format!("test_lib_root_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Game 1 folder
    let g1_dir = temp_dir.join("CyberGame");
    fs::create_dir_all(&g1_dir).unwrap();
    fs::write(g1_dir.join("game.exe"), b"GAME1").unwrap();

    // Ignored folder (e.g. tools)
    let tools_dir = temp_dir.join("tools");
    fs::create_dir_all(&tools_dir).unwrap();
    fs::write(tools_dir.join("tool.exe"), b"TOOL").unwrap();

    let scanned = scan_library_root(&temp_dir);
    assert_eq!(scanned.len(), 1, "Only genuine game folder should be included");
    assert_eq!(scanned[0].name, "CyberGame");
    assert_eq!(scanned[0].launcher, "My folders");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_has_native_dlss_unmodded_game_with_historical_backup_is_native() {
    let temp_dir = std::env::temp_dir().join(format!("test_native_dlss_hist_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    let dx12_exe = temp_dir.join("Control_DX12.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..118].copy_from_slice(b"D3D12CreateDevice\x00");
    fs::write(&dx12_exe, &exe_bytes).unwrap();
    fs::write(temp_dir.join("nvngx_dlss.dll"), b"GENUINE_DLSS_BINARY_CONTENT").unwrap();

    // Create a leftover _DLSS5_Backup directory with past done manifests and added_history from an old test
    let bdir = temp_dir.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();
    
    let history = serde_json::json!({
        "files": ["nvngx.dll", "dxgi.dll", "dlss-5-studio-feeder.addon64"],
        "dirs": []
    });
    fs::write(bdir.join("added_history.json"), serde_json::to_vec(&history).unwrap()).unwrap();

    let past_done = serde_json::json!({
        "version": 1,
        "date": "2026-09-30",
        "route": "native",
        "replaced": [],
        "added": ["dxgi.dll", "ReShade.ini"]
    });
    fs::write(bdir.join("manifest.json.done-1790881035375"), serde_json::to_vec(&past_done).unwrap()).unwrap();

    let scanned = scan_game_directory(&temp_dir).expect("Control synthetic directory must scan");
    assert!(scanned.has_native_dlss(), "Unmodded game with genuine nvngx_dlss.dll must be recognized as native DLSS");
    assert!(scanned.has_native_upscaler(), "Must have native upscaler");

    let adv = dlss_studio::core::advisories::get_native_dlss_advisory(&scanned);
    assert!(adv.is_none(), "DLSS 5 Direct advisory must NOT warn on genuine unmodded DX12 game with native DLSS");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_has_native_dlss_active_mod_adding_dlss_is_not_native() {
    let temp_dir = std::env::temp_dir().join(format!("test_mod_added_dlss_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    let dx12_exe = temp_dir.join("game.exe");
    let mut exe_bytes = vec![0u8; 10000];
    exe_bytes[100..118].copy_from_slice(b"D3D12CreateDevice\x00");
    fs::write(&dx12_exe, &exe_bytes).unwrap();
    fs::write(temp_dir.join("nvngx_dlss.dll"), b"MOD_INJECTED_DLSS").unwrap();

    // Active manifest showing nvngx_dlss.dll was added to a game that never had it
    let bdir = temp_dir.join("_DLSS5_Backup");
    fs::create_dir_all(&bdir).unwrap();
    let manifest = dlss_studio::core::journal::ActiveManifest {
        version: 1,
        date: "now".to_string(),
        route: "feeder".to_string(),
        game: None,
        game_exe: Some("game.exe".to_string()),
        backup_prefix: None,
        replaced: Vec::new(),
        added: vec!["nvngx_dlss.dll".to_string()],
        added_dirs: Vec::new(),
        mfg_unlock: None,
        mfg_multiplier: None,
        nr_style_enabled: None,
        nr_style: None,
        opti_presr: None,
        opti_passes: None,
    };
    fs::write(bdir.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();

    let scanned = scan_game_directory(&temp_dir).expect("Game must scan");
    assert!(!scanned.has_native_dlss(), "Active mod adding nvngx_dlss.dll must NOT be recognized as native DLSS");

    let _ = fs::remove_dir_all(&temp_dir);
}

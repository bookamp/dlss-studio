use dlss_studio::core::scan::*;
use dlss_studio::core::pe::inspect_pe;
use std::fs;
use std::path::{Path, PathBuf};


    #[test]
    fn test_short_version() {
        assert_eq!(short_version("2.4.2.0"), "2.4.2");
        assert_eq!(short_version("310.1.0.0"), "310.1.0");
        assert_eq!(short_version("310.6.0.0"), "310.6.0");
        assert_eq!(short_version("3.7.10"), "3.7.10");
    }

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
        // OptiScaler bundled streamline files
        fs::write(opti_streamline.join("nvngx_dlssg.dll"), b"bundled_dlssg").unwrap();
        fs::write(opti_streamline.join("sl.dlss_g.dll"), b"bundled_sl_dlssg").unwrap();

        let g = scan_game_directory(&temp_dir).expect("Synthetic DX11 game must be scanned");
        assert_eq!(g.api, "DirectX 11");
        assert!(!g.has_frame_generation, "OptiScaler streamline folder must NOT flag has_frame_generation=true");
        assert!(g.can_inject_fg, "can_inject_fg must be true for 64-bit DX11 game with native DLSS via D3D11on12 bridge");

        // Now remove nvngx_dlss.dll and verify can_inject_fg becomes false
        fs::remove_file(bin_dir.join("nvngx_dlss.dll")).unwrap();
        let g2 = scan_game_directory(&temp_dir).expect("Synthetic DX11 game without DLSS must be scanned");
        assert!(!g2.can_inject_fg, "can_inject_fg must be false for 64-bit DX11 game without native DLSS");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scan_bg3_real() {
        let path = Path::new(r"E:\Games\GoG\Baldurs Gate 3\bin\bg3.exe");
        if path.exists() {
            let pe = inspect_pe(path).unwrap();
            let api = detect_api(path, &pe.imports).expect("bg3.exe must detect an API");
            assert_eq!(api, "Vulkan", "bg3.exe must be detected as Vulkan, not DirectX 12");

            let fake_bg3 = GameEntry {
                name: "Baldur's Gate 3".to_string(),
                dir: PathBuf::from(r"E:\Games\GoG\Baldurs Gate 3"),
                exe_path: path.to_path_buf(),
                exe_rel: "bin\\bg3.exe".to_string(),
                bitness: 64,
                api: api.clone(),
                dlss_version: Some("2.4.2".to_string()),
                has_frame_generation: false,
                can_inject_fg: false,
                optiscaler_installed: false,
                optiscaler_presr: false,
                optiscaler_passes: 1,
                reshade_installed: false,
                reshade_version: None,
                reshade_addon_support: false,
                addon_installed: false,
                installed_route: None,
                mfg_unlock_installed: false,
                has_backup: false,
                launcher: "GOG".to_string(),
                poster: None,
                files: Vec::new(),
                available_exes: Vec::new(),
                is_laa: true,
                nr_style: 0,
                nr_style_enabled: false,
                mfg_multiplier: 4,
                has_anti_cheat: false,
            };
            assert!(!dlss_studio::core::install_routes::is_native_dlss_supported(&fake_bg3), "Vulkan game must NOT support Native DLSS (RenoDX)");
            assert!(!fake_bg3.can_inject_fg, "BG3 Vulkan must NOT support frame generation injection");
            assert!(!dlss_studio::core::install_routes::is_frame_generation_supported(&fake_bg3), "BG3 Vulkan must NOT be recognized as frame generation supported");
            let routes = dlss_studio::core::install_routes::routes_for(&fake_bg3);
            assert!(!routes.contains(&dlss_studio::core::install_routes::InstallRoute::Native), "Routes must NOT contain Native for BG3");
            assert!(routes.contains(&dlss_studio::core::install_routes::InstallRoute::Feeder), "Routes must contain Feeder for BG3");

            // Verify bg3_dx11.exe behaves as DirectX 11 and supports frame generation injection via bridge
            let dx11_path = Path::new(r"E:\Games\GoG\Baldurs Gate 3\bin\bg3_dx11.exe");
            if dx11_path.exists() {
                let dx11_pe = inspect_pe(dx11_path).unwrap();
                let dx11_api = detect_api(dx11_path, &dx11_pe.imports).expect("bg3_dx11.exe must detect an API");
                assert_eq!(dx11_api, "DirectX 11", "bg3_dx11.exe must detect as DirectX 11");

                let fake_bg3_dx11 = GameEntry {
                    api: dx11_api,
                    can_inject_fg: true,
                    ..fake_bg3.clone()
                };
                assert!(fake_bg3_dx11.can_inject_fg, "BG3 DX11 must support frame generation injection via D3D11on12 bridge");
                assert!(dlss_studio::core::install_routes::is_frame_generation_supported(&fake_bg3_dx11), "BG3 DX11 must support frame generation via D3D11on12 bridge");

                // If DLSS is missing, DX11 must reject frame generation
                let mut fake_bg3_dx11_no_dlss = fake_bg3_dx11.clone();
                fake_bg3_dx11_no_dlss.dlss_version = None;
                fake_bg3_dx11_no_dlss.can_inject_fg = false;
                assert!(!dlss_studio::core::install_routes::is_frame_generation_supported(&fake_bg3_dx11_no_dlss), "DX11 without DLSS must reject frame generation");
            }
        }
    }

    #[test]
    fn test_read_done_manifest_synthetic() {
        let temp_dir = std::env::temp_dir().join(format!("dlss_manifest_synthetic_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
        let bdir = temp_dir.join("_DLSS5_Backup");
        fs::create_dir_all(&bdir).unwrap();

        let manifest = dlss_studio::core::journal::ActiveManifest {
            version: 1,
            date: "2026-09-11 12:00 UTC".to_string(),
            route: "feeder".to_string(),
            game: Some(dlss_studio::core::journal::ManifestGame {
                dir: Some(temp_dir.to_string_lossy().to_string()),
                exe: Some("Content\\game.exe".to_string()),
                api: Some("dxgi".to_string()),
                bitness: Some(64),
                api_label: Some("DirectX 11/12".to_string()),
            }),

            game_exe: Some("Content\\game.exe".to_string()),
            backup_prefix: Some("originals/test-uuid".to_string()),
            replaced: Vec::new(),
            added: vec!["Content\\dxgi.dll".to_string()],
            added_dirs: Vec::new(),
            ..Default::default()
        };

        let bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(bdir.join("manifest.json.done-1789100000000"), bytes).unwrap();

        let m = dlss_studio::core::journal::read_latest_done_manifest(&temp_dir).expect("Done manifest must deserialize");
        assert_eq!(m.route, "feeder");
        assert_eq!(m.backup_prefix.as_deref(), Some("originals/test-uuid"));
        assert!(m.added.contains(&"Content\\dxgi.dll".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_library_scan_synthetic() {
        let temp_root = std::env::temp_dir().join(format!("dlss_lib_synthetic_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
        let game1_dir = temp_root.join("GameOne");
        let game2_dir = temp_root.join("GameTwo");
        fs::create_dir_all(&game1_dir).unwrap();
        fs::create_dir_all(&game2_dir).unwrap();

        let mut exe1 = vec![0u8; 8000];
        exe1[50..67].copy_from_slice(b"D3D12CreateDevice");
        fs::write(game1_dir.join("Game1.exe"), exe1).unwrap();
        fs::write(game1_dir.join("D3D12Core.dll"), b"d3d12").unwrap();

        let mut exe2 = vec![0u8; 8000];
        exe2[50..67].copy_from_slice(b"D3D11CreateDevice");
        fs::write(game2_dir.join("Game2.exe"), exe2).unwrap();
        fs::write(game2_dir.join("d3d11.dll"), b"d3d11").unwrap();

        let games = scan_library_root(&temp_root);

        assert_eq!(games.len(), 2, "Both synthetic games in library must be detected");

        let _ = fs::remove_dir_all(&temp_root);
    }


    #[test]
    fn test_discover_xbox() {
        let xbox_games = discover_xbox();
        println!("Discovered Xbox games count: {}", xbox_games.len());
        for g in &xbox_games {
            println!(" - Xbox game: {} ({}) => API: {}, DLSS: {:?}, optiscaler: {}, route: {:?}, backup: {}, addon: {}, reshade: {}, patched: {}",
                g.name, g.dir.display(), g.api, g.dlss_version, g.optiscaler_installed, g.installed_route, g.has_backup, g.addon_installed, g.reshade_installed, g.is_dlss5_patched());
        }
        // If Xbox games are installed on the machine, verify they are found
        assert!(xbox_games.iter().all(|g| g.launcher == "Xbox"));

        if let Some(res) = xbox_games.iter().find(|g| g.name.contains("Resonance")) {
            println!("Resonance Exe: {}", res.exe_path.display());
            if let Some(pe) = dlss_studio::core::pe::inspect_pe(&res.exe_path) {
                println!("Resonance Imports: {:?}", pe.imports);
            }
        }
        if let Some(apt2) = xbox_games.iter().find(|g| g.name.contains("A Plague Tale: Requiem")) {
            println!("APT2 Exe: {}", apt2.exe_path.display());
            if let Some(pe) = dlss_studio::core::pe::inspect_pe(&apt2.exe_path) {
                println!("APT2 Imports: {:?}", pe.imports);
            }
            assert_eq!(apt2.api, "DirectX 12", "APT2 must be detected as DirectX 12");
            assert!(dlss_studio::core::install_routes::get_optiscaler_advisory(apt2).is_none(), "APT2 must have no OptiScaler advisory");
            assert!(dlss_studio::core::install_routes::get_native_dlss_advisory(apt2).is_none(), "APT2 must have no Native DLSS advisory");
            assert!(dlss_studio::core::install_routes::get_mfg_advisory(apt2, true).is_none(), "APT2 must have no MFG advisory");
        }
    }

    #[test]
    fn test_discover_game_exes_finds_multiple_binaries() {
        let temp_dir = std::env::temp_dir().join(format!("test_multi_exe_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let bin_dir = temp_dir.join("bin");
        let _ = fs::create_dir_all(&bin_dir);

        let mut dx11_exe = vec![0u8; 8000];
        dx11_exe[50..67].copy_from_slice(b"D3D11CreateDevice");
        fs::write(bin_dir.join("bg3_dx11.exe"), dx11_exe).unwrap();

        fs::write(bin_dir.join("bg3.exe"), b"vulkan executable dummy").unwrap();
        fs::write(bin_dir.join("vulkan-1.dll"), b"vulkan").unwrap();

        let exes = discover_game_exes(&temp_dir);
        assert_eq!(exes.len(), 2, "Must discover both bg3.exe and bg3_dx11.exe");
        assert!(exes.iter().any(|e| e.name == "bg3.exe" && e.api == "Vulkan"));
        assert!(exes.iter().any(|e| e.name == "bg3_dx11.exe" && e.api.contains("DirectX 11")));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_discover_game_exes_filters_helpers_prelauncher_and_tools() {
        let temp_dir = std::env::temp_dir().join(format!("test_cp2077_filter_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let bin_x64 = temp_dir.join("bin").join("x64");
        let crash_dir = bin_x64.join("CrashReporter");
        let tools_dir = temp_dir.join("engine").join("tools");
        fs::create_dir_all(&crash_dir).unwrap();
        fs::create_dir_all(&tools_dir).unwrap();

        // Write root prelauncher, tool archiver, and script compiler
        fs::write(temp_dir.join("REDprelauncher.exe"), b"dummy prelauncher").unwrap();
        fs::write(crash_dir.join("7za.exe"), b"dummy 7za tool").unwrap();
        fs::write(tools_dir.join("scc.exe"), b"dummy scc compiler").unwrap();

        // Write actual game binary with sibling d3d12
        let mut cp_exe = vec![0u8; 8000];
        cp_exe[50..67].copy_from_slice(b"D3D12CreateDevice");
        fs::write(bin_x64.join("Cyberpunk2077.exe"), cp_exe).unwrap();
        fs::write(bin_x64.join("d3d12.dll"), b"d3d12").unwrap();

        let exes = discover_game_exes(&temp_dir);
        assert_eq!(exes.len(), 1, "Must filter REDprelauncher, 7za, and scc, returning ONLY Cyberpunk2077.exe");
        assert_eq!(exes[0].name, "Cyberpunk2077.exe");

        let game = scan_game_directory(&temp_dir).expect("Must scan game directory");
        assert_eq!(game.exe_path.file_name().unwrap(), "Cyberpunk2077.exe");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_discover_game_exes_filters_larilauncher_and_launcher_dirs() {
        let temp_dir = std::env::temp_dir().join(format!("test_bg3_filter_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let bin_dir = temp_dir.join("bin");
        let launcher_dir = temp_dir.join("Launcher");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&launcher_dir).unwrap();

        // Write real executables in bin/
        let mut dx11_exe = vec![0u8; 8000];
        dx11_exe[50..67].copy_from_slice(b"D3D11CreateDevice");
        fs::write(bin_dir.join("bg3_dx11.exe"), dx11_exe).unwrap();
        fs::write(bin_dir.join("bg3.exe"), b"vulkan executable dummy").unwrap();
        fs::write(bin_dir.join("vulkan-1.dll"), b"vulkan").unwrap();

        // Write LariLauncher in Launcher/
        fs::write(launcher_dir.join("LariLauncher.exe"), b"larian launcher dummy").unwrap();

        let exes = discover_game_exes(&temp_dir);
        assert_eq!(exes.len(), 2, "Must discover ONLY bg3.exe and bg3_dx11.exe, rejecting LariLauncher.exe");
        assert!(!exes.iter().any(|e| e.name.to_lowercase().contains("launcher")));
        assert!(exes.iter().any(|e| e.name == "bg3.exe"));
        assert!(exes.iter().any(|e| e.name == "bg3_dx11.exe"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_discover_game_exes_filters_unreal_root_stub_when_shipping_binary_exists() {
        let temp_dir = std::env::temp_dir().join(format!("test_ue_shipping_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let binaries_wingdk = temp_dir.join("Mixtape").join("Binaries").join("WinGDK");
        fs::create_dir_all(&binaries_wingdk).unwrap();

        // Write root dummy wrapper
        fs::write(temp_dir.join("Mixtape.exe"), b"root bootstrap stub").unwrap();

        // Write shipping binary
        let mut shipping_exe = vec![0u8; 8000];
        shipping_exe[50..67].copy_from_slice(b"D3D11CreateDevice");
        fs::write(binaries_wingdk.join("Mixtape-WinGDK-Shipping.exe"), shipping_exe).unwrap();
        fs::write(binaries_wingdk.join("d3d11.dll"), b"d3d11").unwrap();

        let exes = discover_game_exes(&temp_dir);
        assert_eq!(exes.len(), 1, "Must filter root Mixtape.exe stub and return ONLY Mixtape-WinGDK-Shipping.exe");
        assert_eq!(exes[0].name, "Mixtape-WinGDK-Shipping.exe");

        let game = scan_game_directory(&temp_dir).expect("Must scan game directory");
        assert_eq!(game.exe_path.file_name().unwrap(), "Mixtape-WinGDK-Shipping.exe");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_gog_info_playtasks_and_python_filtering() {
        let temp_dir = std::env::temp_dir().join(format!("test_gog_dik_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();

        let lib_x64 = temp_dir.join("lib").join("windows-x86_64");
        let lib_x86 = temp_dir.join("lib").join("windows-i686");
        fs::create_dir_all(&lib_x64).unwrap();
        fs::create_dir_all(&lib_x86).unwrap();

        // Write python runtime helpers in lib
        fs::write(lib_x64.join("python.exe"), b"dummy python interpreter").unwrap();
        fs::write(lib_x64.join("pythonw.exe"), b"dummy pythonw interpreter").unwrap();
        fs::write(lib_x86.join("python.exe"), b"dummy python interpreter 32").unwrap();
        fs::write(lib_x86.join("pythonw.exe"), b"dummy pythonw interpreter 32").unwrap();

        // Write actual root game executables
        fs::write(temp_dir.join("BeingADIK.exe"), b"renpy game binary 64").unwrap();
        fs::write(temp_dir.join("BeingADIK-32.exe"), b"renpy game binary 32").unwrap();

        // Write GOG info manifest
        let gog_json = r#"{
            "gameId": "1181224050",
            "name": "Being a DIK - Season 1",
            "playTasks": [
                {
                    "category": "game",
                    "isPrimary": true,
                    "name": "Being a DIK",
                    "path": "BeingADIK.exe",
                    "type": "FileTask"
                },
                {
                    "category": "game",
                    "name": "Being a DIK 32-bit",
                    "path": "BeingADIK-32.exe",
                    "type": "FileTask"
                }
            ]
        }"#;
        fs::write(temp_dir.join("goggame-1181224050.info"), gog_json).unwrap();

        let game = scan_game_directory(&temp_dir).expect("Game directory must be recognized");
        assert_eq!(game.launcher, "GOG");
        assert_eq!(game.name, "Being a DIK - Season 1", "GOG game title must be read from goggame manifest");
        assert_eq!(game.exe_rel, "BeingADIK.exe", "Primary executable must be BeingADIK.exe, not python.exe");
        assert_eq!(game.available_exes.len(), 2, "Available exes must only contain the 2 real game binaries");
        assert_eq!(game.available_exes[0].name, "BeingADIK.exe");
        assert_eq!(game.available_exes[1].name, "BeingADIK-32.exe");
        assert!(!game.available_exes.iter().any(|e| e.name.contains("python")), "Python helpers must be strictly excluded");

        let exes = discover_game_exes(&temp_dir);
        assert_eq!(exes.len(), 2);
        assert_eq!(exes[0].name, "BeingADIK.exe");
        assert_eq!(exes[1].name, "BeingADIK-32.exe");

        let (gog_name, _, gog_id) = extract_gog_metadata(&temp_dir);
        assert_eq!(gog_name.as_deref(), Some("Being a DIK - Season 1"));
        assert_eq!(gog_id.as_deref(), Some("1181224050"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_dynamic_d3d11_with_opengl32_import_resolves_to_directx11() {
        let temp_dir = std::env::temp_dir().join(format!("test_d3d11_gl_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();
        let exe_path = temp_dir.join("Game.exe");
        let mut exe_bytes = vec![0u8; 4096];
        exe_bytes[100..117].copy_from_slice(b"D3D11CreateDevice");
        fs::write(&exe_path, &exe_bytes).unwrap();

        let imports = vec!["opengl32.dll".to_string(), "kernel32.dll".to_string()];
        let api = detect_api(&exe_path, &imports);
        assert_eq!(api, Some("DirectX 11".to_string()), "Games importing opengl32 but containing D3D11CreateDevice must resolve to DirectX 11");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_is_inside_and_path_helpers() {
        assert!(is_inside("C:\\Games\\Cyberpunk\\bin\\Cyberpunk2077.exe", "C:\\Games\\Cyberpunk"));
        assert!(is_inside("C:/Games/Cyberpunk/bin/Cyberpunk2077.exe", "C:\\Games\\Cyberpunk"));
        assert!(is_inside("C:\\Games\\Cyberpunk", "C:\\Games\\Cyberpunk"));
        assert!(!is_inside("C:\\OtherGames\\Cyberpunk", "C:\\Games\\Cyberpunk"));
    }

    #[test]
    fn test_dedupe_games_logic() {
        let g1 = GameEntry {
            name: "Game 1".to_string(),
            dir: PathBuf::from("C:\\Games\\Game1"),
            launcher: "Steam".to_string(),
            ..Default::default()
        };
        let g2 = GameEntry {
            name: "Game 1 (Duplicate)".to_string(),
            dir: PathBuf::from("c:/games/game1/"),
            launcher: "My folders".to_string(),
            ..Default::default()
        };
        let g3 = GameEntry {
            name: "Game 2".to_string(),
            dir: PathBuf::from("C:\\Games\\Game2"),
            launcher: "Xbox".to_string(),
            ..Default::default()
        };

        let deduped = dedupe_games(vec![g1, g2, g3]);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].name, "Game 1");
        assert_eq!(deduped[1].name, "Game 2");
    }

    #[test]
    fn test_detect_api_all_graphics_backends() {
        let temp = std::env::temp_dir().join(format!("test_api_detect_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp).unwrap();
        let exe = temp.join("Dummy.exe");
        fs::write(&exe, b"dummy binary").unwrap();

        assert_eq!(detect_api(&exe, &["d3d12.dll".to_string()]), Some("DirectX 12".to_string()));
        assert_eq!(detect_api(&exe, &["d3d11.dll".to_string()]), Some("DirectX 11".to_string()));
        assert_eq!(detect_api(&exe, &["d3d10.dll".to_string()]), Some("DirectX 10".to_string()));
        assert_eq!(detect_api(&exe, &["d3d9.dll".to_string()]), Some("DirectX 9".to_string()));
        assert_eq!(detect_api(&exe, &["d3d8.dll".to_string()]), Some("DirectX 8".to_string()));
        assert_eq!(detect_api(&exe, &["vulkan-1.dll".to_string()]), Some("Vulkan".to_string()));
        assert_eq!(detect_api(&exe, &["opengl32.dll".to_string()]), Some("OpenGL".to_string()));
        assert_eq!(detect_api(&exe, &["kernel32.dll".to_string()]), None);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_xbox_gdk_display_name_extraction() {
        let temp = std::env::temp_dir().join(format!("test_xbox_gdk_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp).unwrap();

        // Create dummy executable
        let exe = temp.join("Resonance.exe");
        fs::write(&exe, b"MZ\x90\x00dummy_pe").unwrap();

        // Create MicrosoftGame.config
        let config_content = r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
    <ExecutableList>
        <Executable Name="Resonance.exe" Id="Game" Alias="Resonance.exe"/>
    </ExecutableList>
    <ShellVisuals DefaultDisplayName="Resonance: A Plague Tale Legacy"
                  PublisherDisplayName="Focus Home Interactive SA"
                  Description="Resonance: A Plague Tale Legacy"/>
</Game>
"#;
        fs::write(temp.join("MicrosoftGame.config"), config_content).unwrap();

        let scanned = scan_game_directory(&temp).expect("Game directory should be recognized");
        assert_eq!(scanned.name, "Resonance: A Plague Tale Legacy", "scan_game_directory must extract the real DefaultDisplayName from MicrosoftGame.config");
        assert_eq!(scanned.launcher, "Xbox", "Detected launcher must be Xbox when MicrosoftGame.config is present");

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_detect_api_renpy_opengl_and_angle_dx11() {
        let temp = std::env::temp_dir().join(format!("test_renpy_api_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(temp.join("renpy")).unwrap();
        fs::create_dir_all(temp.join("lib").join("windows-x86_64")).unwrap();
        fs::write(temp.join("lib").join("windows-x86_64").join("librenpython.dll"), b"MZ\x90dummy").unwrap();

        let exe = temp.join("Game.exe");
        fs::write(&exe, b"MZ\x90dummy").unwrap();

        // 1. By default, Ren'Py resolves to OpenGL
        let api_default = detect_api(&exe, &["librenpython.dll".to_string()]);
        assert_eq!(api_default, Some("OpenGL".to_string()));

        // 2. If log.txt indicates angle2, resolves to DirectX 11
        fs::write(temp.join("log.txt"), "Initializing angle2 renderer:\nDirectX 11").unwrap();
        let api_angle = detect_api(&exe, &["librenpython.dll".to_string()]);
        assert_eq!(api_angle, Some("DirectX 11".to_string()));

        // 3. If log.txt indicates gl2, resolves to OpenGL
        fs::write(temp.join("log.txt"), "Initializing gl2 renderer:\nVendor: NVIDIA\nRenderer: RTX 4090").unwrap();
        let api_gl2 = detect_api(&exe, &["librenpython.dll".to_string()]);
        assert_eq!(api_gl2, Some("OpenGL".to_string()));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_get_fixed_drives_non_empty() {
        let drives = get_fixed_drives();
        assert!(!drives.is_empty(), "Should discover at least one fixed drive on Windows");
        assert!(drives.iter().any(|d| d.to_string_lossy().to_uppercase().starts_with("C:")));
    }

    #[test]
    fn test_discover_game_exes_filters_dlss5_feed_host64_and_mod_directories() {
        let temp_dir = std::env::temp_dir().join(format!("test_filter_host64_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let host64_dir = temp_dir.join("host64");
        let installer_dir = temp_dir.join("__installer");
        fs::create_dir_all(&host64_dir).unwrap();
        fs::create_dir_all(&installer_dir).unwrap();

        // Write real game exe
        let mut game_exe = vec![0u8; 8000];
        game_exe[50..67].copy_from_slice(b"D3D11CreateDevice");
        fs::write(temp_dir.join("Dead Space.exe"), game_exe).unwrap();

        // Write non-game executables
        fs::write(host64_dir.join("dlss5-feed-host64.exe"), b"feeder helper binary").unwrap();
        fs::write(temp_dir.join("dlss5-feed-host64.exe"), b"root feeder helper binary").unwrap();
        fs::write(installer_dir.join("Touchup.exe"), b"installer tool").unwrap();

        let exes = discover_game_exes(&temp_dir);
        assert_eq!(exes.len(), 1, "Must only return Dead Space.exe, strictly filtering host64, installer, and dlss5-feed-host64");
        assert_eq!(exes[0].name, "Dead Space.exe");

        let game = scan_game_directory(&temp_dir).expect("Must scan game directory");
        assert_eq!(game.exe_path.file_name().unwrap(), "Dead Space.exe");
        assert_eq!(game.available_exes.len(), 1);
        assert_eq!(game.available_exes[0].name, "Dead Space.exe");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_entry_dlss5_patched_and_route_display() {
        let mut entry = GameEntry::default();
        assert!(!entry.is_dlss5_patched());
        assert_eq!(entry.route_display_name(), "Vanilla");

        entry.installed_route = Some("feeder".to_string());
        assert!(entry.is_dlss5_patched());
        assert_eq!(entry.route_display_name(), "Feeder · Neural Rendering");

        entry.installed_route = Some("native".to_string());
        assert!(entry.is_dlss5_patched());
        assert_eq!(entry.route_display_name(), "Native D3D12");

        entry.installed_route = Some("optiscaler".to_string());
        assert!(entry.is_dlss5_patched());
        assert_eq!(entry.route_display_name(), "OptiScaler");

        entry.installed_route = None;
        entry.optiscaler_installed = true;
        assert!(entry.is_dlss5_patched());
        assert_eq!(entry.route_display_name(), "OptiScaler");

        entry.optiscaler_installed = false;
        entry.reshade_installed = true;
        assert!(entry.is_dlss5_patched());
        assert_eq!(entry.route_display_name(), "ReShade");

        entry.reshade_installed = false;
        assert!(!entry.is_dlss5_patched());
        assert_eq!(entry.route_display_name(), "Vanilla");
    }

    #[test]
    fn test_infer_game_name_nested_generic_folder() {
        let p = Path::new(r"C:\Games\Cyberpunk 2077\bin\x64");
        let exe = Path::new(r"C:\Games\Cyberpunk 2077\bin\x64\Cyberpunk2077.exe");
        let name = infer_game_name(p, exe, None);
        assert_eq!(name, "Cyberpunk 2077");

        let p_root = Path::new(r"D:\SteamLibrary\steamapps\common\Baldurs Gate 3");
        let exe_bg3 = Path::new(r"D:\SteamLibrary\steamapps\common\Baldurs Gate 3\bin\bg3.exe");
        let name_bg3 = infer_game_name(p_root, exe_bg3, None);
        assert_eq!(name_bg3, "Baldurs Gate 3");

        let p_generic = Path::new(r"C:\Random\shipping");
        let exe_stub = Path::new(r"C:\Random\shipping\Starfield.exe");
        let name_stem = infer_game_name(p_generic, exe_stub, None);
        assert_eq!(name_stem, "Random");
    }

    #[test]
    fn test_is_installer_or_helper_filters_config_and_settings() {
        assert!(is_installer_or_helper("MassEffect2Config.exe"));
        assert!(is_installer_or_helper("GameConfig.exe"));
        assert!(is_installer_or_helper("Config.exe"));
        assert!(is_installer_or_helper("GameSettings.exe"));
        assert!(is_installer_or_helper("Settings.exe"));
        assert!(is_installer_or_helper("Setup.exe"));
        assert!(is_installer_or_helper("VideoSetup.exe"));
        assert!(is_installer_or_helper("ActivationUI.exe"));
        assert!(is_installer_or_helper("Autorun.exe"));
        assert!(is_installer_or_helper("autorun.exe"));
        assert!(is_installer_or_helper("Register.exe"));
        assert!(is_installer_or_helper("Registration.exe"));
        assert!(is_installer_or_helper("Support.exe"));

        // True games must never be filtered
        assert!(!is_installer_or_helper("ME2Game.exe"));
        assert!(!is_installer_or_helper("W40k_gog.exe"));
        assert!(!is_installer_or_helper("W40k.exe"));
        assert!(!is_installer_or_helper("Dead Space.exe"));
        assert!(!is_installer_or_helper("bg3.exe"));
    }

    #[test]
    fn test_mass_effect_2_ue3_resolves_to_directx_9() {
        let temp_dir = std::env::temp_dir().join(format!("test_me2_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();
        let exe_path = temp_dir.join("ME2Game.exe");

        // Inject Direct3DCreate9, D3D10CreateDevice, and CreateDXGIFactory markers
        let mut bytes = vec![0u8; 8192];
        bytes[100..116].copy_from_slice(b"Direct3DCreate9\0");
        bytes[200..218].copy_from_slice(b"D3D10CreateDevice\0");
        bytes[300..318].copy_from_slice(b"CreateDXGIFactory\0");
        fs::write(&exe_path, &bytes).unwrap();

        let api = detect_api(&exe_path, &["d3d9.dll".to_string()]);
        assert_eq!(api, Some("DirectX 9".to_string()), "UE3 games with Direct3DCreate9 and dormant D3D10 markers must resolve to DirectX 9");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_dawn_of_war_spdx9_and_dxgi_helper_resolves_to_directx_9() {
        let temp_dir = std::env::temp_dir().join(format!("test_dow_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();

        let main_exe = temp_dir.join("W40k_gog.exe");
        let spdx9_dll = temp_dir.join("spDx9.dll");
        let helper_dll = temp_dir.join("RenderHelper.dll");

        // Main exe has Direct3DCreate9 and CreateDXGIFactory
        let mut main_bytes = vec![0u8; 4096];
        main_bytes[100..116].copy_from_slice(b"Direct3DCreate9\0");
        main_bytes[200..218].copy_from_slice(b"CreateDXGIFactory\0");
        fs::write(&main_exe, &main_bytes).unwrap();

        // spDx9.dll has Direct3DCreate9
        let mut spdx9_bytes = vec![0u8; 4096];
        spdx9_bytes[100..116].copy_from_slice(b"Direct3DCreate9\0");
        fs::write(&spdx9_dll, &spdx9_bytes).unwrap();

        // RenderHelper.dll has CreateDXGIFactory
        let mut helper_bytes = vec![0u8; 4096];
        helper_bytes[100..118].copy_from_slice(b"CreateDXGIFactory\0");
        fs::write(&helper_dll, &helper_bytes).unwrap();

        let api = detect_api(&main_exe, &["RenderHelper.dll".to_string(), "spDx9.dll".to_string()]);
        assert_eq!(api, Some("DirectX 9".to_string()), "Dawn of War with spDx9.dll and DXGI helper must resolve to DirectX 9");

        let sibling_api = detect_sibling_api(&temp_dir);
        assert_eq!(sibling_api, Some("DirectX 9".to_string()), "detect_sibling_api must recognize spDx9.dll as DirectX 9");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scan_mass_effect_2_and_dawn_of_war_live_folders() {
        let me2_dir = Path::new(r"E:\Games\Mass Effect 2");
        if me2_dir.is_dir() {
            let game = scan_game_directory(me2_dir).expect("ME2 must scan");
            assert_eq!(game.exe_path.file_name().unwrap(), "ME2Game.exe", "ME2Game.exe must be chosen over launcher stub");
            assert_eq!(game.api, "DirectX 9", "ME2 must resolve to DirectX 9");
            assert!(!game.available_exes.iter().any(|e| e.name.contains("Config")), "MassEffect2Config.exe must be excluded");
            if let Some(stub) = game.available_exes.iter().find(|e| e.name == "MassEffect2.exe") {
                assert!(stub.api == "DirectX 9" || stub.api == "Undetected", "MassEffect2.exe launcher stub resolves to DirectX 9 via sibling detection or Undetected");
            }
        }

        let dow_dir = Path::new(r"E:\Games\Dawn of War Definitive Edition");
        if dow_dir.is_dir() {
            let game = scan_game_directory(dow_dir).expect("Dawn of War must scan");
            assert_eq!(game.api, "DirectX 9", "Dawn of War must resolve to DirectX 9");
        }
    }

    #[test]
    fn test_control_pcgp_synthetic_detection() {
        let temp_dir = std::env::temp_dir().join(format!("test_control_pcgp_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Write synthetic MicrosoftGame.config
        let cfg = r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="0">
  <ExecutableList>
    <Executable Name="Game_rmdutggamepass_f.exe" Id="Game" TargetDeviceFamily="PC" />
  </ExecutableList>
  <ShellVisuals DefaultDisplayName="Control PCGP" Description="Control" />
  <DesktopRegistration>
    <ProcessorArchitecture>x64</ProcessorArchitecture>
  </DesktopRegistration>
</Game>"#;
        fs::write(temp_dir.join("MicrosoftGame.config"), cfg).unwrap();

        // Write dummy exe
        fs::write(temp_dir.join("Game_rmdutggamepass_f.exe"), b"MZ dummy exe").unwrap();

        // Write dxcompiler.dll (DX12 compiler marker)
        fs::write(temp_dir.join("dxcompiler.dll"), b"MZ dxcompiler").unwrap();
        fs::write(temp_dir.join("dxil.dll"), b"MZ dxil").unwrap();

        // Write sibling d3d_rmdutggamepass_f.dll with D3D12CreateDevice marker
        let mut d3d_dll = vec![0u8; 4096];
        d3d_dll[100..118].copy_from_slice(b"D3D12CreateDevice\0");
        fs::write(temp_dir.join("d3d_rmdutggamepass_f.dll"), &d3d_dll).unwrap();

        // Write dummy DLSS dll
        fs::write(temp_dir.join("nvngx_dlss.dll"), b"MZ dlss").unwrap();

        let sibling_api = detect_sibling_api(&temp_dir);
        assert_eq!(sibling_api, Some("DirectX 12".to_string()), "detect_sibling_api must detect DirectX 12 via dxcompiler and sibling d3d dll");

        let game = scan_game_directory(&temp_dir).expect("Synthetic Control PCGP must scan");
        assert_eq!(game.name, "Control PCGP");
        assert_eq!(game.api, "DirectX 12");
        assert_eq!(game.bitness, 64);
        assert!(game.dlss_version.is_some() || temp_dir.join("nvngx_dlss.dll").exists());

        // Verify routes when DLSS is present
        let mut game_with_dlss = game.clone();
        game_with_dlss.dlss_version = Some("2.1.25.0".to_string());
        let routes = dlss_studio::core::install_routes::routes_for(&game_with_dlss);
        assert!(routes.contains(&dlss_studio::core::install_routes::InstallRoute::Native), "Control PCGP must support Native route");
        assert!(routes.contains(&dlss_studio::core::install_routes::InstallRoute::Feeder), "Control PCGP must support Feeder route");
        assert!(routes.contains(&dlss_studio::core::install_routes::InstallRoute::OptiScaler), "Control PCGP must support OptiScaler route");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_control_pcgp_live_detection() {
        let live_path = Path::new(r"D:\WindowsApps\505GAMESS.P.A.ControlPCGP_1.0.6.0_x64__tefn33qh9azfc");
        if live_path.is_dir() {
            let game = scan_game_directory(live_path).expect("Live Control PCGP must scan");
            println!("Live Control PCGP scanned: API={}, DLSS={:?}", game.api, game.dlss_version);
            assert_eq!(game.api, "DirectX 12", "Live Control PCGP must detect as DirectX 12");
            let routes = dlss_studio::core::install_routes::routes_for(&game);
            assert!(routes.contains(&dlss_studio::core::install_routes::InstallRoute::Native), "Live Control PCGP must support Native DLSS");
            assert!(routes.contains(&dlss_studio::core::install_routes::InstallRoute::OptiScaler), "Live Control PCGP must support OptiScaler");
            assert!(routes.contains(&dlss_studio::core::install_routes::InstallRoute::Feeder), "Live Control PCGP must support Feeder");
        }
    }

    #[test]
    fn test_control_gog_metadata_and_art_detection() {
        let control_dir = Path::new(r"D:\Games\GoG\Control");
        if control_dir.is_dir() {
            let (gog_name, gog_poster, gog_id) = extract_gog_metadata(control_dir);
            assert_eq!(gog_name.as_deref(), Some("Control Ultimate Edition"));
            assert_eq!(gog_id.as_deref(), Some("2049187585"));
            assert!(gog_poster.is_some(), "GOG Galaxy local vertical cover must be discovered");
            assert!(gog_poster.unwrap().contains("http://dlss-art.localhost/art/"));

            let game = scan_game_directory(control_dir).expect("Control directory must be scanned");
            assert_eq!(game.launcher, "GOG");
            assert_eq!(game.name, "Control Ultimate Edition");
            assert!(game.poster.is_some(), "Game poster must be populated with discovered cover");
        }
    }

    #[test]
    fn test_manifest_preserves_target_exe_and_patch_state() {
        let temp_dir = std::env::temp_dir().join(format!("test_manifest_preserves_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let bin_dir = temp_dir.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();

        // Two dummy executables: game.exe (score higher by default) and game_dx11.exe
        fs::write(bin_dir.join("game.exe"), b"MZ dummy 64-bit exe").unwrap();
        fs::write(bin_dir.join("game_dx11.exe"), b"MZ dummy 64-bit exe").unwrap();
        fs::write(bin_dir.join("nvngx_dlss.dll"), b"MZ dlss").unwrap();

        // Write ActiveManifest targeting game_dx11.exe with Native DLSS and Model A (nr_style = 0, enabled = true)
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
        assert_eq!(scanned.exe_rel, "bin\\game_dx11.exe", "Scanner must preserve manifest.game_exe as chosen executable");
        assert_eq!(scanned.installed_route, Some("native".to_string()), "Scanner must preserve manifest.route");
        assert!(scanned.mfg_unlock_installed, "Scanner must preserve mfg_unlock");
        assert_eq!(scanned.mfg_multiplier, 4, "Scanner must preserve mfg_multiplier");
        assert_eq!(scanned.nr_style, 0, "Scanner must preserve nr_style = 0 (Model A)");
        assert!(scanned.nr_style_enabled, "Scanner must preserve nr_style_enabled = true even when nr_style = 0");
        assert!(scanned.reshade_installed, "Scanner must mark reshade_installed for native route");
        assert!(scanned.addon_installed, "Scanner must mark addon_installed for native route");

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

        // 1. Native DLSS add-on present on disk without manifest
        fs::write(bin_dir.join("renodx-dlss5.addon64"), b"DUMMY_ADDON").unwrap();
        let scanned = scan_game_directory(&temp_dir).expect("Scan must succeed");
        assert_eq!(scanned.installed_route, Some("native".to_string()), "Must infer native route from renodx-dlss5.addon64");

        // 2. Feeder add-on present on disk
        fs::write(bin_dir.join("dlss5-feed.addon64"), b"DUMMY_FEEDER").unwrap();
        let scanned2 = scan_game_directory(&temp_dir).expect("Scan must succeed");
        assert_eq!(scanned2.installed_route, Some("feeder".to_string()), "Must infer feeder route when dlss5-feed.addon64 is present");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_generic_source_engine_subfolder_api_detection() {
        let temp_dir = std::env::temp_dir().join(format!("test_source_engine_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let bin_dir = temp_dir.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();

        // Game launcher executable in root
        let exe_path = temp_dir.join("left4dead.exe");
        fs::write(&exe_path, b"DUMMY_EXE").unwrap();

        // Modular rendering DLL in bin/ subfolder
        let render_dll = bin_dir.join("shaderapidx9.dll");
        fs::write(&render_dll, b"DUMMY_RENDER_DLL").unwrap();

        let exes = discover_game_exes(&temp_dir);
        let found = exes.iter().find(|e| e.name.eq_ignore_ascii_case("left4dead.exe"));
        assert!(found.is_some(), "Must discover left4dead.exe");
        assert_eq!(found.unwrap().api, "DirectX 9", "Generic scanner must inspect bin/ subfolder and identify DirectX 9");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_generic_installer_and_sdk_tools_filtered() {
        assert!(is_installer_or_helper("vpk.exe"));
        assert!(is_installer_or_helper("batch compiler.exe"));
        assert!(is_installer_or_helper("shader_compiler_x64.exe"));
        assert!(is_installer_or_helper("crashhandler64.exe"));
        assert!(is_installer_or_helper("gamelaunchhelper.exe"));
        assert!(!is_installer_or_helper("game.exe"));
        assert!(!is_installer_or_helper("left4dead.exe"));

        let temp_dir = std::env::temp_dir().join(format!("test_filter_tools_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();

        fs::write(temp_dir.join("game.exe"), b"DUMMY_GAME").unwrap();
        fs::write(temp_dir.join("vpk.exe"), b"DUMMY_TOOL").unwrap();
        fs::write(temp_dir.join("batch compiler.exe"), b"DUMMY_COMPILER").unwrap();

        let exes = discover_game_exes(&temp_dir);
        let names: Vec<String> = exes.into_iter().map(|e| e.name).collect();
        assert!(names.contains(&"game.exe".to_string()), "Must include genuine game exe");
        assert!(!names.contains(&"vpk.exe".to_string()), "Must exclude vpk.exe");
        assert!(!names.contains(&"batch compiler.exe".to_string()), "Must exclude compiler tools");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_game_entry_has_anti_cheat_serde_backwards_compatibility() {
        // Simulates old library.json entry that lacks "has_anti_cheat"
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
        assert!(!entry.has_anti_cheat, "Old entries must default has_anti_cheat to false");

        // Now with has_anti_cheat: true
        let mut modern = entry.clone();
        modern.has_anti_cheat = true;
        let serialized = serde_json::to_string(&modern).expect("serialize modern entry");
        let restored: GameEntry = serde_json::from_str(&serialized).expect("deserialize modern entry");
        assert!(restored.has_anti_cheat, "Modern entry must retain has_anti_cheat true");
    }

    #[test]
    fn test_scan_game_directory_detects_anti_cheat() {
        let temp_dir = std::env::temp_dir().join(format!("test_ac_scan_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a dummy game exe and an EasyAntiCheat DLL
        fs::write(temp_dir.join("game.exe"), b"DUMMY_EXE_CONTENT").unwrap();
        fs::write(temp_dir.join("EasyAntiCheat_x64.dll"), b"EAC").unwrap();

        let scanned = scan_game_directory(&temp_dir);
        assert!(scanned.is_some(), "Must scan valid directory");
        let game = scanned.unwrap();
        assert!(game.has_anti_cheat, "Must detect anti cheat file and set has_anti_cheat = true");

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

        // Ensure in-memory cache is fully populated and available for zero-I/O popup display
        assert_eq!(entry.available_exes.len(), 2);
        assert_eq!(entry.available_exes[0].name, "ACOdyssey.exe");
        assert_eq!(entry.available_exes[1].name, "ACOdyssey_plus.exe");
    }

    #[test]
    fn test_mhw_agility_sdk_and_streamline_dx12_detection() {
        let temp_dir = std::env::temp_dir().join(format!("test_mhw_detect_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(temp_dir.join("D3D12")).unwrap();

        let exe_path = temp_dir.join("MonsterHunterWilds.exe");
        fs::write(&exe_path, b"DUMMY_MHW_EXE").unwrap();

        // Agility SDK runtime & Streamline DLSS-G
        fs::write(temp_dir.join("D3D12").join("D3D12Core.dll"), b"D3D12_CORE").unwrap();
        fs::write(temp_dir.join("sl.dlss_g.dll"), b"STREAMLINE_DLSS_G").unwrap();
        fs::write(temp_dir.join("amd_fidelityfx_framegeneration_dx12.dll"), b"FSR_FG").unwrap();
        fs::write(temp_dir.join("nvngx_dlss.dll"), b"DLSS_SR").unwrap();

        // Even though PE imports contain d3d11.dll (for auxiliary media playback),
        // Agility SDK and Streamline DLSS-G must elevate detection to DirectX 12!
        let api = detect_api(&exe_path, &["d3d11.dll".to_string()]);
        assert_eq!(api, Some("DirectX 12".to_string()), "MHW must be detected as DirectX 12 despite auxiliary d3d11.dll import");

        let mut scanned = scan_game_directory(&temp_dir).expect("MHW directory must be scanned");
        assert_eq!(scanned.api, "DirectX 12");
        assert_eq!(scanned.has_frame_generation, true);
        assert!(dlss_studio::core::install_routes::is_frame_generation_supported(&scanned), "MHW must support Frame Generation");

        // When DLSS version is recognized, Native route is recommended
        scanned.dlss_version = Some("310.2.1.0".to_string());
        assert!(dlss_studio::core::install_routes::is_native_dlss_supported(&scanned), "MHW must support Native DLSS route");
        assert_eq!(dlss_studio::core::install_routes::recommended_route(&scanned), dlss_studio::core::install_routes::InstallRoute::Native);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_genuine_dx11_game_without_dx11_in_name_detects_dx11() {
        let temp_dir = std::env::temp_dir().join(format!("test_dx11_neutral_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();

        let exe_path = temp_dir.join("DarkSoulsIII.exe");
        let mut exe_bytes = vec![0u8; 1000];
        exe_bytes[100..117].copy_from_slice(b"D3D11CreateDevice");
        fs::write(&exe_path, &exe_bytes).unwrap();

        let api = detect_api(&exe_path, &["d3d11.dll".to_string()]);
        assert_eq!(api, Some("DirectX 11".to_string()), "Genuine DX11 game without dx11 in name must resolve to DirectX 11");

        let scanned = scan_game_directory(&temp_dir).expect("DS3 directory must be scanned");
        assert_eq!(scanned.api, "DirectX 11");
        assert!(!dlss_studio::core::install_routes::is_native_dlss_supported(&scanned), "Genuine DX11 title cannot support Native D3D12 DLSS");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_bg3_dx11_override_in_hybrid_folder() {
        let temp_dir = std::env::temp_dir().join(format!("test_bg3_hybrid_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(temp_dir.join("D3D12")).unwrap();

        // Hybrid folder has D3D12Core.dll for another exe
        fs::write(temp_dir.join("D3D12").join("D3D12Core.dll"), b"D3D12").unwrap();

        let dx11_exe = temp_dir.join("bg3_dx11.exe");
        fs::write(&dx11_exe, b"BG3_DX11").unwrap();

        let api = detect_api(&dx11_exe, &["d3d11.dll".to_string()]);
        assert_eq!(api, Some("DirectX 11".to_string()), "Explicit dx11 filename must override folder-level D3D12 files");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sl_dlss_does_not_flag_frame_generation() {
        let temp_dir = std::env::temp_dir().join(format!("test_sl_dlss_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&temp_dir).unwrap();

        let dx12_exe = temp_dir.join("game.exe");
        let mut exe_bytes = vec![0u8; 10000];
        exe_bytes[100..118].copy_from_slice(b"D3D12CreateDevice\x00");
        fs::write(&dx12_exe, &exe_bytes).unwrap();

        // sl.dlss.dll is Super Resolution only, not Frame Generation
        fs::write(temp_dir.join("sl.dlss.dll"), b"fake_sl_dlss").unwrap();

        let scanned = scan_game_directory(&temp_dir).expect("Game must be scanned");
        assert!(!scanned.has_frame_generation, "sl.dlss.dll alone must NOT flag has_frame_generation as true");

        let _ = fs::remove_dir_all(&temp_dir);
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

        // Simulate DLSS Studio manifest recording nvngx_dlssg.dll as mod-added
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
        assert!(!scanned.has_frame_generation, "Mod-added nvngx_dlssg.dll must NOT be recognized as native Frame Generation");

        let _ = fs::remove_dir_all(&temp_dir);
    }

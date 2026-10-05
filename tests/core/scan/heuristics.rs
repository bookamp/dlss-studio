use dlss_studio::core::scan::heuristics::*;
use std::fs;

#[test]
fn test_short_version() {
    assert_eq!(short_version("2.4.2.0"), "2.4.2");
    assert_eq!(short_version("310.1.0.0"), "310.1.0");
    assert_eq!(short_version("310.6.0.0"), "310.6.0");
    assert_eq!(short_version("3.7.10"), "3.7.10");
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
fn test_generic_source_engine_subfolder_api_detection() {
    let temp_dir = std::env::temp_dir().join(format!("test_source_engine_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let bin_dir = temp_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let exe_path = temp_dir.join("left4dead.exe");
    fs::write(&exe_path, b"DUMMY_EXE").unwrap();

    let render_dll = bin_dir.join("shaderapidx9.dll");
    fs::write(&render_dll, b"DUMMY_RENDER_DLL").unwrap();

    let exes = discover_game_exes(&temp_dir);
    let found = exes.iter().find(|e| e.name.eq_ignore_ascii_case("left4dead.exe"));
    assert!(found.is_some(), "Must discover left4dead.exe");
    assert_eq!(found.unwrap().api, "DirectX 9", "Generic scanner must inspect bin/ subfolder and identify DirectX 9");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mhw_agility_sdk_and_streamline_dx12_detection() {
    let temp_dir = std::env::temp_dir().join(format!("test_mhw_detect_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(temp_dir.join("D3D12")).unwrap();

    let exe_path = temp_dir.join("MonsterHunterWilds.exe");
    fs::write(&exe_path, b"DUMMY_MHW_EXE").unwrap();

    fs::write(temp_dir.join("D3D12").join("D3D12Core.dll"), b"D3D12_CORE").unwrap();
    fs::write(temp_dir.join("sl.dlss_g.dll"), b"STREAMLINE_DLSS_G").unwrap();
    fs::write(temp_dir.join("amd_fidelityfx_framegeneration_dx12.dll"), b"FSR_FG").unwrap();
    fs::write(temp_dir.join("nvngx_dlss.dll"), b"DLSS_SR").unwrap();

    let api = detect_api(&exe_path, &["d3d11.dll".to_string()]);
    assert_eq!(api, Some("DirectX 12".to_string()), "MHW must be detected as DirectX 12 despite auxiliary d3d11.dll import");

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

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_bg3_dx11_override_in_hybrid_folder() {
    let temp_dir = std::env::temp_dir().join(format!("test_bg3_hybrid_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(temp_dir.join("D3D12")).unwrap();

    fs::write(temp_dir.join("D3D12").join("D3D12Core.dll"), b"D3D12").unwrap();

    let dx11_exe = temp_dir.join("bg3_dx11.exe");
    fs::write(&dx11_exe, b"BG3_DX11").unwrap();

    let api = detect_api(&dx11_exe, &["d3d11.dll".to_string()]);
    assert_eq!(api, Some("DirectX 11".to_string()), "Explicit dx11 filename must override folder-level D3D12 files");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_is_not_a_game_title_heuristics() {
    assert!(is_not_a_game_title("Steamworks Shared"));
    assert!(is_not_a_game_title("Proton Experimental"));
    assert!(is_not_a_game_title("Cyberpunk 2077 Original Soundtrack"));
    assert!(is_not_a_game_title("Visual C++ Redistributable"));
    assert!(!is_not_a_game_title("Cyberpunk 2077"));
    assert!(!is_not_a_game_title("Baldur's Gate 3"));
}

#[test]
fn test_infer_game_name_formats() {
    let dir = std::path::Path::new("C:\\Games\\Cyberpunk 2077");
    let exe = std::path::Path::new("C:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe");
    assert_eq!(infer_game_name(dir, exe, None), "Cyberpunk 2077");

    let exe_root = std::path::Path::new("C:\\Games\\Cyberpunk 2077\\game.exe");
    assert_eq!(infer_game_name(dir, exe_root, None), "Cyberpunk 2077");

    assert_eq!(infer_game_name(dir, exe, Some("Custom Xbox Name".to_string())), "Custom Xbox Name");
}

#[test]
fn test_matches_store_filter_logic() {
    assert!(matches_store_filter("Steam", "All"));
    assert!(matches_store_filter("Steam", "Steam"));
    assert!(!matches_store_filter("Steam", "Epic Games"));

    assert!(matches_store_filter("Epic Games", "Epic"));
    assert!(matches_store_filter("Xbox", "Xbox Game Pass"));
    assert!(matches_store_filter("Added by hand", "Custom"));
    assert!(matches_store_filter("Ubisoft", "My Folders"));
}

#[test]
fn test_detect_api_vulkan_and_opengl() {
    let temp_dir = std::env::temp_dir().join(format!("test_vk_gl_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    let vk_exe = temp_dir.join("vulkan_game.exe");
    fs::write(&vk_exe, b"VK_GAME").unwrap();
    let api_vk = detect_api(&vk_exe, &["vulkan-1.dll".to_string()]);
    assert_eq!(api_vk, Some("Vulkan".to_string()));

    let gl_exe = temp_dir.join("opengl_game.exe");
    fs::write(&gl_exe, b"OPENGL_GAME").unwrap();
    let api_gl = detect_api(&gl_exe, &["opengl32.dll".to_string()]);
    assert_eq!(api_gl, Some("OpenGL".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_is_helper_or_tool_path_components() {
    use std::path::Path;
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\host64\\helper.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\optiscaler\\opti.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\_dlss5_backup\\original.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\reshade-shaders\\test.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\tools\\editor.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\compiler\\compile.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\sdk\\tool.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\easyanticheat\\eac.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\battleye\\be.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\__installer\\setup.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\support\\help.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\redist\\vc.exe")));
    assert!(is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\launcher\\start.exe")));
    assert!(!is_helper_or_tool_path(Path::new("C:\\Games\\Cyberpunk\\bin\\x64\\Cyberpunk2077.exe")));
}

#[test]
fn test_is_not_a_game_dir_variations() {
    assert!(is_not_a_game_dir("steamapps"));
    assert!(is_not_a_game_dir("workshop"));
    assert!(is_not_a_game_dir("shadercache"));
    assert!(is_not_a_game_dir("backup"));
    assert!(is_not_a_game_dir("_dlss5_backup"));
    assert!(is_not_a_game_dir("reshade-shaders"));
    assert!(is_not_a_game_dir("node_modules"));
    assert!(is_not_a_game_dir(".git"));
    assert!(is_not_a_game_dir("sdk"));
    assert!(is_not_a_game_dir("patcher"));
    assert!(is_not_a_game_dir("vortex_mods"));
    assert!(is_not_a_game_dir("fluffy_modmanager"));
    assert!(is_not_a_game_dir("my_trainer"));
    assert!(!is_not_a_game_dir("Cyberpunk 2077"));
    assert!(!is_not_a_game_dir("Elden Ring"));
}

#[test]
fn test_is_library_container_name() {
    assert!(is_library_container_name("Games"));
    assert!(is_library_container_name("My Games"));
    assert!(is_library_container_name("SteamLibrary"));
    assert!(is_library_container_name("GOG Games"));
    assert!(is_library_container_name("Epic Games"));
    assert!(is_library_container_name("Xbox Games"));
    assert!(is_library_container_name("Repacks"));
    assert!(is_library_container_name("Emulation"));
    assert!(!is_library_container_name("Witcher 3"));
}

#[test]
fn test_holds_game_depth_and_structure() {
    let temp_dir = std::env::temp_dir().join(format!("test_holds_game_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    // Empty dir holds no game
    assert!(!holds_game(&temp_dir, 2));

    // Only tool / installer holds no game
    fs::write(temp_dir.join("unins000.exe"), b"UNINSTALLER").unwrap();
    assert!(!holds_game(&temp_dir, 2));

    // Genuine exe at root
    fs::write(temp_dir.join("game.exe"), b"GAME").unwrap();
    assert!(holds_game(&temp_dir, 0));
    assert!(holds_game(&temp_dir, 2));

    // Nested genuine exe
    let nested = temp_dir.join("nested_folder");
    fs::create_dir_all(&nested).unwrap();
    fs::remove_file(temp_dir.join("game.exe")).unwrap();
    fs::write(nested.join("game.exe"), b"GAME").unwrap();

    assert!(!holds_game(&temp_dir, 0), "Depth 0 should not inspect nested");
    assert!(holds_game(&temp_dir, 1), "Depth 1 should inspect nested");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_api_from_names_all_directx_versions() {
    assert_eq!(api_from_names(&["d3d12.dll".to_string()]), Some("DirectX 12".to_string()));
    assert_eq!(api_from_names(&["vulkan-1.dll".to_string()]), Some("Vulkan".to_string()));
    assert_eq!(api_from_names(&["d3d11.dll".to_string()]), Some("DirectX 11".to_string()));
    assert_eq!(api_from_names(&["d3d9.dll".to_string()]), Some("DirectX 9".to_string()));
    assert_eq!(api_from_names(&["d3d10.dll".to_string()]), Some("DirectX 10".to_string()));
    assert_eq!(api_from_names(&["d3d10_1.dll".to_string()]), Some("DirectX 10".to_string()));
    assert_eq!(api_from_names(&["dxgi.dll".to_string()]), Some("DirectX (DXGI)".to_string()));
    assert_eq!(api_from_names(&["d3d8.dll".to_string()]), Some("DirectX 8".to_string()));
    assert_eq!(api_from_names(&["kernel32.dll".to_string()]), None);
}

#[test]
fn test_detect_renpy_api_scenarios() {
    let temp_dir = std::env::temp_dir().join(format!("test_renpy_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(temp_dir.join("renpy")).unwrap();

    // Default with no log.txt => OpenGL
    assert_eq!(detect_renpy_api(&temp_dir), Some("OpenGL".to_string()));

    // With log.txt indicating DirectX 11
    fs::write(temp_dir.join("log.txt"), "Renderer: ANGLE2 D3D11 mode activated").unwrap();
    assert_eq!(detect_renpy_api(&temp_dir), Some("DirectX 11".to_string()));

    // With log.txt indicating DirectX 9
    fs::write(temp_dir.join("log.txt"), "Renderer: ANGLE Direct3D 9 backend").unwrap();
    assert_eq!(detect_renpy_api(&temp_dir), Some("DirectX 9".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_detect_sibling_api_dxc_and_subdirectories() {
    let temp_dir = std::env::temp_dir().join(format!("test_sibling_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();

    // DXC compiler in root triggers DirectX 12
    fs::write(temp_dir.join("dxcompiler.dll"), b"DXC").unwrap();
    assert_eq!(detect_sibling_api(&temp_dir), Some("DirectX 12".to_string()));

    fs::remove_file(temp_dir.join("dxcompiler.dll")).unwrap();

    // Streamline dlss_g triggers DirectX 12
    fs::write(temp_dir.join("sl.dlss_g.dll"), b"SL").unwrap();
    assert_eq!(detect_sibling_api(&temp_dir), Some("DirectX 12".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_infer_game_name_deep_generic_folders() {
    use std::path::Path;
    let dir = Path::new("C:\\Games\\Hades II");
    let exe = Path::new("C:\\Games\\Hades II\\shipping\\win64\\release\\Hades2.exe");
    assert_eq!(infer_game_name(dir, exe, None), "Hades II");

    let fallback_exe = Path::new("C:\\Games\\super_cool_game.exe");
    assert_eq!(infer_game_name(Path::new("C:\\Games"), fallback_exe, None), "super cool game");
}


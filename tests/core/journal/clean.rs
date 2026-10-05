use dlss_studio::core::journal::clean::*;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_clean_untracked_mods() {
    let temp = TempDir::new("clean");

    let opti_ini = temp.join("OptiScaler.ini");
    let reshade_dll = temp.join("ReShade64.dll");
    let addon = temp.join("renodx-mfgunlock.addon64");
    let safe_file = temp.join("Game.exe");
    let proxy_dxgi = temp.join("dxgi.dll");

    fs::write(&opti_ini, b"ini").unwrap();
    fs::write(&reshade_dll, b"dll").unwrap();
    fs::write(&addon, b"addon").unwrap();
    fs::write(&safe_file, b"exe").unwrap();

    let mut proxy_bytes = vec![0u8; 100_000];
    proxy_bytes[50_000..50_010].copy_from_slice(b"OptiScaler");
    fs::write(&proxy_dxgi, proxy_bytes).unwrap();

    let removed = clean_untracked_mods(temp.path()).unwrap();
    assert_eq!(removed.len(), 4);
    assert!(!opti_ini.exists());
    assert!(!reshade_dll.exists());
    assert!(!addon.exists());
    assert!(!proxy_dxgi.exists(), "Proxy dxgi.dll must be removed");
    assert!(safe_file.exists(), "Original game executable must not be deleted");
}

#[test]
fn test_clean_untracked_mods_nested_reshade() {
    let temp = TempDir::new("clean_nested");
    let nested_dir = temp.join("bin").join("x64");
    fs::create_dir_all(&nested_dir).unwrap();

    let nested_exe = nested_dir.join("MockCyberpunk2077.exe");
    let reshade_dxgi = nested_dir.join("dxgi.dll");
    let renodx_addon = nested_dir.join("renodx-dlss5.addon64");
    let mfg_addon = nested_dir.join("renodx-mfgunlock.addon64");
    let reshade_ini = nested_dir.join("ReShade.ini");

    fs::write(&nested_exe, b"MZ dummy exe").unwrap();
    fs::write(&renodx_addon, b"addon").unwrap();
    fs::write(&mfg_addon, b"mfg").unwrap();
    fs::write(&reshade_ini, b"[INPUT]\nKeyOverlay=36").unwrap();

    let mut reshade_bytes = vec![0u8; 100_000];
    reshade_bytes[50_000..50_007].copy_from_slice(b"ReShade");
    fs::write(&reshade_dxgi, reshade_bytes).unwrap();

    let removed = clean_untracked_mods_with_exe(temp.path(), Some(&nested_exe)).unwrap();
    assert!(removed.contains(&"dxgi.dll".to_string()));
    assert!(removed.contains(&"renodx-dlss5.addon64".to_string()));
    assert!(removed.contains(&"renodx-mfgunlock.addon64".to_string()));
    assert!(removed.contains(&"ReShade.ini".to_string()));

    assert!(!reshade_dxgi.exists(), "Nested dxgi.dll must be cleaned");
    assert!(!renodx_addon.exists(), "Nested renodx-dlss5.addon64 must be cleaned");
    assert!(!mfg_addon.exists(), "Nested renodx-mfgunlock.addon64 must be cleaned");
    assert!(!reshade_ini.exists(), "Nested ReShade.ini must be cleaned");
    assert!(nested_exe.exists(), "Game executable must remain intact");
}

#[test]
fn test_running_game_guard_prevents_clean() {
    let procs = dlss_studio::core::install_guards::get_running_processes();
    if procs.is_empty() {
        return;
    }
    let running_proc_name = &procs[0].name;

    let temp = TempDir::new("guard_clean");
    let running_dummy_exe = temp.join(running_proc_name);
    fs::write(&running_dummy_exe, b"MZ").unwrap();

    let clean_res = clean_untracked_mods_with_exe(temp.path(), Some(&running_dummy_exe));
    assert!(clean_res.is_err(), "Cleaning must be blocked when game process is active");
    let err_str = clean_res.unwrap_err().to_string();
    assert!(err_str.contains("Close the game"), "Error must tell user to close the game first: {}", err_str);
}

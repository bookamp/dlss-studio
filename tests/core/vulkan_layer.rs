use dlss_studio::core::vulkan_layer::{
    get_vulkan_layer_storage_dir, is_valid_64bit_pe, read_registered_games, register_vulkan_layer,
    save_registered_games, unregister_vulkan_layer,
};
use crate::common::TempDir;
use std::fs;
use std::path::Path;

#[test]
fn test_vulkan_layer_storage_dir_creation() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let dir = get_vulkan_layer_storage_dir();
    assert!(dir.is_dir());
}

#[test]
fn test_read_and_save_registered_games() {
    let temp = TempDir::new("vk");
    let games = vec!["C:\\Games\\Game1".to_string(), "C:\\Games\\Game2".to_string()];
    save_registered_games(temp.path(), &games).unwrap();

    let read = read_registered_games(temp.path());
    assert_eq!(read, games);
}

#[test]
fn test_auto_generate_vulkan_manifest_when_missing() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("vk_gen");
    let dummy_dll = temp.write_file("ReShade64.dll", b"dummy reshade dll");

    let storage_dir = get_vulkan_layer_storage_dir();
    let target_json = storage_dir.join("ReShade64.json");
    // Remove existing target_json if any to test auto-generation
    let backup = if target_json.is_file() {
        fs::read_to_string(&target_json).ok()
    } else {
        None
    };
    let _ = fs::remove_file(&target_json);

    let game_dir = temp.create_dir_all("GameDir");

    let res = register_vulkan_layer(&game_dir, None, Some(&dummy_dll));
    assert!(res.is_ok(), "register_vulkan_layer must auto-generate manifest successfully");
    assert!(target_json.is_file(), "ReShade64.json must be generated");

    let content = fs::read_to_string(&target_json).unwrap();
    assert!(content.contains("VK_LAYER_reshade"));

    // Clean up test game registration
    let _ = unregister_vulkan_layer(&game_dir);

    // Restore backup if existed
    if let Some(b) = backup {
        let _ = fs::write(&target_json, b);
    } else {
        let _ = fs::remove_file(&target_json);
    }
}

#[test]
fn test_is_valid_64bit_pe_checks() {
    let temp = TempDir::new("pe_vk");

    // Missing file
    assert!(!is_valid_64bit_pe(&temp.join("missing.dll")));

    // Too small (< 100_000 bytes)
    let short_f = temp.write_file("short.dll", vec![0u8; 1024]);
    assert!(!is_valid_64bit_pe(&short_f));

    // Real 64-bit system DLL
    let sys_dll = Path::new("C:\\Windows\\System32\\kernel32.dll");
    if sys_dll.is_file() {
        assert!(is_valid_64bit_pe(sys_dll));
    }
}

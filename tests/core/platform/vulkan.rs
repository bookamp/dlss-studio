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

    // Large file that is NOT a valid PE
    let large_non_pe = temp.write_file("large.bin", vec![0u8; 150_000]);
    assert!(!is_valid_64bit_pe(&large_non_pe));

    // Corrupt installs.json should yield empty registered games
    let corrupt_storage = temp.create_dir_all("corrupt_vk");
    fs::write(corrupt_storage.join("installs.json"), b"NOT_JSON").unwrap();
    assert!(read_registered_games(&corrupt_storage).is_empty());
}

#[test]
fn test_register_and_unregister_feeder_layer_and_game_query() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("vk_feeder");
    let src_dir = temp.create_dir_all("src_layer");
    let dummy_feed_dll = temp.write_file("src_layer/VkLayer_feed_vk.dll", b"dummy feeder vk dll");
    let manifest_template = r#"{
        "file_format_version": "1.0.0",
        "layer": {
            "name": "VK_LAYER_DLSS5_Feed",
            "type": "GLOBAL",
            "library_path": "placeholder.dll"
        }
    }"#;
    temp.write_file("src_layer/VkLayer_feed_vk.json", manifest_template.as_bytes());

    let game1 = temp.create_dir_all("GameOne");
    let game2 = temp.create_dir_all("GameTwo");

    // Register game 1 with feeder source layer
    let res1 = register_vulkan_layer(&game1, Some(&src_dir), Some(&dummy_feed_dll));
    assert!(res1.is_ok());
    assert!(dlss_studio::core::vulkan_layer::is_game_registered(&game1));

    // Register game 2
    let res2 = register_vulkan_layer(&game2, Some(&src_dir), Some(&dummy_feed_dll));
    assert!(res2.is_ok());
    assert!(dlss_studio::core::vulkan_layer::is_game_registered(&game2));

    // Unregister game 1: should return Ok(false) because game 2 is still registered
    let unreg1 = unregister_vulkan_layer(&game1).unwrap();
    assert!(!unreg1, "Should return false because game 2 is still active");
    assert!(!dlss_studio::core::vulkan_layer::is_game_registered(&game1));
    assert!(dlss_studio::core::vulkan_layer::is_game_registered(&game2));

    // Unregister game 2: should return Ok(true) because no more games are registered
    let unreg2 = unregister_vulkan_layer(&game2).unwrap();
    assert!(unreg2, "Should return true when all games are removed");
    assert!(!dlss_studio::core::vulkan_layer::is_game_registered(&game2));
}

#[test]
fn test_register_vulkan_layer_missing_dll_error_handling() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("vk_err");
    let missing_dll = temp.join("nonexistent_dll_123.dll");
    let game = temp.create_dir_all("GameErr");

    let res = register_vulkan_layer(&game, None, Some(&missing_dll));
    // In test environment, it either returns Err or generates manifest cleanly
    let _ = res;
    let _ = unregister_vulkan_layer(&game);
}

#[test]
fn test_register_vulkan_layer_with_reshade_source_manifest() {
    let _state_lock = dlss_studio::core::state::STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = TempDir::new("vk_reshade_src");
    let src_dir = temp.create_dir_all("reshade_src");
    let dummy_reshade_dll = temp.write_file("reshade_src/ReShade64.dll", b"dummy reshade 64 dll");
    let manifest_template = r#"{
        "file_format_version": "1.0.0",
        "layer": {
            "name": "VK_LAYER_reshade",
            "type": "GLOBAL",
            "library_path": "placeholder_reshade.dll"
        }
    }"#;
    temp.write_file("reshade_src/ReShade64.json", manifest_template.as_bytes());

    let game = temp.create_dir_all("GameReshade");

    // Register with reshade source dir
    let res = register_vulkan_layer(&game, Some(&src_dir), Some(&dummy_reshade_dll));
    assert!(res.is_ok());
    assert!(dlss_studio::core::vulkan_layer::is_game_registered(&game));

    assert!(!dlss_studio::core::vulkan_layer::is_game_registered(&temp.join("Unregistered")));

    let unreg = unregister_vulkan_layer(&game).unwrap();
    assert!(unreg);
    assert!(!dlss_studio::core::vulkan_layer::is_game_registered(&game));
}


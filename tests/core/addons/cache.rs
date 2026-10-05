use dlss_studio::core::state::STATE_TEST_MUTEX;
use dlss_studio::core::addons::{
    cache::*,
    AddonId,
};
use std::fs;

#[test]
fn test_components_root_exists() {
    let root = get_components_root();
    assert!(root.is_dir());
}

#[test]
fn test_components_root_and_resolution() {
    let root = get_components_root();
    assert!(root.is_dir());
    let _ = find_local_dgvoodoo_components();
    let _ = find_local_feeder_components();
}

#[test]
fn test_addon_cache_check_does_not_panic() {
    let _ = is_mfg_addon_cached();
    let _ = is_feeder_cached();
    let _ = is_renodx_engine_cached();
    let _ = is_dlss5_d3d12_fix_cached();
    let _ = is_rtxmfg_cached();
    let _ = is_optiscaler_cached();
    let _ = is_streamline_cached();
    let _ = is_reshade_cached();
    let _ = is_dgvoodoo_cached();
    let _ = are_all_mandatory_components_cached();
}

#[test]
fn test_is_addon_cached_matches_individual_checks() {
    let _lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(AddonId::RenoDxDlss5.is_cached(), is_renodx_engine_cached());
    assert_eq!(AddonId::DlssMipFix.is_cached(), is_dlss5_d3d12_fix_cached());
    assert_eq!(AddonId::RenoDxMfgUnlock.is_cached(), is_mfg_addon_cached());
    assert_eq!(AddonId::Dlss5Feeder.is_cached(), is_feeder_cached());
    assert_eq!(AddonId::Rtx40Mfg.is_cached(), is_rtxmfg_cached());
    assert_eq!(AddonId::OptiScaler.is_cached(), is_optiscaler_cached());
    assert_eq!(AddonId::ReShade.is_cached(), is_reshade_cached());
    assert_eq!(AddonId::Streamline.is_cached(), is_streamline_cached());
    assert_eq!(AddonId::DgVoodoo.is_cached(), is_dgvoodoo_cached());
}

#[test]
fn test_find_local_components_executions() {
    let _ = find_local_dgvoodoo_components();
    let _ = find_local_feeder_components();
}

#[test]
fn test_find_local_dgvoodoo_and_feeder_positive_matches() {
    let _lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let root = get_components_root();
    
    // 1. Stage dgVoodoo files
    let dgv_dir = root.join("dgvoodoo_test_mock");
    let _ = fs::create_dir_all(dgv_dir.join("x86"));
    let _ = fs::create_dir_all(dgv_dir.join("x64"));
    let _ = fs::write(dgv_dir.join("x86").join("D3D9.dll"), b"MZ_DUMMY");
    let _ = fs::write(dgv_dir.join("x64").join("D3D9.dll"), b"MZ_DUMMY");
    let _ = fs::write(dgv_dir.join("x86").join("D3D8.dll"), b"MZ_DUMMY");
    let _ = fs::write(dgv_dir.join("dgVoodoo.conf"), b"[General]");

    // 2. Stage Feeder files
    let feeder_dir = root.join("DLSS5-Feeder-1.16.0-beta.3");
    let shaders_dir = root.join("feeder-shaders").join("Shaders");
    let textures_dir = root.join("feeder-shaders").join("Textures");
    let _ = fs::create_dir_all(&feeder_dir);
    let _ = fs::create_dir_all(&shaders_dir);
    let _ = fs::create_dir_all(&textures_dir);
    let _ = fs::write(feeder_dir.join("dlss5-feed.addon64"), b"ADDON");
    let _ = fs::write(shaders_dir.join("DLSS5_Feed.fx"), b"// FX");
    let _ = fs::write(shaders_dir.join("DrawText.fxh"), b"// DRAWTEXT");
    let _ = fs::write(shaders_dir.join("vort_Motion.fx"), b"// MOTION");
    let _ = fs::write(textures_dir.join("vort_BlueNoise.png"), b"PNG_MOCK");

    let feeder_res = find_local_feeder_components();
    let is_cached = is_feeder_cached();

    // Clean up mock files
    let _ = fs::remove_dir_all(&dgv_dir);
    let _ = fs::remove_dir_all(&feeder_dir);
    let _ = fs::remove_dir_all(root.join("feeder-shaders"));

    assert!(feeder_res.is_some());
    assert!(is_cached);
}

#[test]
fn test_dynamic_feeder_version_selection_prefers_newest() {
    let _lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let root = get_components_root();
    let old_feeder = root.join("DLSS5-Feeder-1.16.0-beta.3");
    let new_feeder = root.join("DLSS5-Feeder-1.18.0-beta.1");
    let shaders_dir = root.join("feeder-shaders").join("Shaders");
    let textures_dir = root.join("feeder-shaders").join("Textures");

    let _ = fs::create_dir_all(&old_feeder);
    let _ = fs::create_dir_all(&new_feeder);
    let _ = fs::create_dir_all(&shaders_dir);
    let _ = fs::create_dir_all(&textures_dir);

    let _ = fs::write(old_feeder.join("dlss5-feed.addon64"), b"OLD_FEES");
    let _ = fs::write(new_feeder.join("dlss5-feed.addon64"), b"NEW_FEES");
    let _ = fs::write(shaders_dir.join("DLSS5_Feed.fx"), b"// FX");
    let _ = fs::write(shaders_dir.join("DrawText.fxh"), b"// DRAWTEXT");
    let _ = fs::write(textures_dir.join("vort_BlueNoise.png"), b"PNG_MOCK");

    let res = find_local_feeder_components();
    assert!(res.is_some());
    let fc = res.unwrap();
    // The resolved addon64 should point to the 1.18.0-beta.1 folder, not the old 1.16.0
    assert!(fc.addon64.to_string_lossy().contains("DLSS5-Feeder-1.18.0-beta.1"));

    // Cleanup
    let _ = fs::remove_dir_all(&old_feeder);
    let _ = fs::remove_dir_all(&new_feeder);
    let _ = fs::remove_dir_all(root.join("feeder-shaders"));
}

#[test]
fn test_multilingual_addon_log_formatting() {
    let raw = "@{log_addon_updating|DLSS 5 Feeder|v1.18.0-beta.1}";
    let en_formatted = dlss_studio::core::i18n::format_log_entry("en", raw);
    assert_eq!(en_formatted, "Updating add-on DLSS 5 Feeder to v1.18.0-beta.1...");

    let de_formatted = dlss_studio::core::i18n::format_log_entry("de", raw);
    assert_eq!(de_formatted, "Aktualisiere Add-On DLSS 5 Feeder auf v1.18.0-beta.1...");

    let fr_formatted = dlss_studio::core::i18n::format_log_entry("fr", raw);
    assert_eq!(fr_formatted, "Mise à jour du module DLSS 5 Feeder vers v1.18.0-beta.1...");

    let raw_success = "@{log_addon_updated|OptiScaler|v0.8.91}";
    let zh_formatted = dlss_studio::core::i18n::format_log_entry("zh", raw_success);
    assert_eq!(zh_formatted, "插件 OptiScaler 已成功更新至 v0.8.91");
}

#[test]
fn test_feeder_cache_rejects_missing_textures() {
    let _lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let temp = crate::common::TempDir::new("feeder_cache_missing_tex");
    let root = temp.path().to_path_buf();
    let feeder_dir = root.join("DLSS5-Feeder-1.18.0-beta.1");
    let shaders_dir = root.join("feeder-shaders").join("Shaders");

    let _ = fs::create_dir_all(&feeder_dir);
    let _ = fs::create_dir_all(&shaders_dir);
    let _ = fs::write(feeder_dir.join("dlss5-feed.addon64"), b"ADDON");
    let _ = fs::write(shaders_dir.join("DLSS5_Feed.fx"), b"// FX");

    // Textures/vort_BlueNoise.png intentionally omitted
    let res = find_feeder_components_in_dir(&root);
    assert!(res.is_none(), "Must reject feeder-shaders cache when vort_BlueNoise.png is missing");
}

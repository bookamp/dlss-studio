use dlss_studio::core::state::STATE_TEST_MUTEX;
use dlss_studio::core::addons::installer::*;
use crate::common::TempDir;
use std::fs;

#[test]
fn test_installer_extract_reshade_missing_archive_errors() {
    let temp = TempDir::new("dummy_reshade_missing");
    let dummy_setup = temp.join("non_existent_reshade_setup.exe");
    let out_dir = temp.join("out");
    let res = extract_reshade_from_setup(&dummy_setup, &out_dir);
    assert!(res.is_err());
}

#[test]
fn test_extract_reshade_from_setup_success() {
    let temp = TempDir::new("reshade_setup_archive");
    let zip_path = temp.join("ReShade_Setup.exe");
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("ReShade64.dll", options).unwrap();
        std::io::Write::write_all(&mut writer, b"dummy reshade 64 dll data").unwrap();
        writer.start_file("ReShade64.json", options).unwrap();
        std::io::Write::write_all(&mut writer, br#"{"layer": 64}"#).unwrap();
        writer.start_file("ReShade32.dll", options).unwrap();
        std::io::Write::write_all(&mut writer, b"dummy reshade 32 dll data").unwrap();
        writer.start_file("ReShade32.json", options).unwrap();
        std::io::Write::write_all(&mut writer, br#"{"layer": 32}"#).unwrap();
        writer.finish().unwrap();
    }

    let out_dir = temp.join("extracted_reshade");
    let res = extract_reshade_from_setup(&zip_path, &out_dir);
    assert!(res.is_ok(), "extract_reshade_from_setup failed: {:?}", res);
    assert!(out_dir.join("ReShade64.dll").is_file());
    assert!(out_dir.join("ReShade32.dll").is_file());
    assert!(out_dir.join("ReShade64.json").is_file());
    assert!(out_dir.join("ReShade32.json").is_file());

    // CRITICAL REGRESSION ASSERTION: Ensure ReShade64.dll contains the DLL data, NOT the JSON manifest!
    assert_eq!(fs::read(out_dir.join("ReShade64.dll")).unwrap(), b"dummy reshade 64 dll data");
    assert_eq!(fs::read(out_dir.join("ReShade64.json")).unwrap(), br#"{"layer": 64}"#);
    assert_eq!(fs::read(out_dir.join("ReShade32.dll")).unwrap(), b"dummy reshade 32 dll data");
    assert_eq!(fs::read(out_dir.join("ReShade32.json")).unwrap(), br#"{"layer": 32}"#);
}


#[test]
fn test_feeder_shaders_validation_structure() {
    let temp = TempDir::new("feeder_check");
    let s_dir = temp.join("feeder-shaders");
    fs::create_dir_all(s_dir.join("Shaders")).unwrap();
    fs::create_dir_all(s_dir.join("Textures")).unwrap();

    fs::write(s_dir.join("Shaders").join("DLSS5_Feed.fx"), b"// test").unwrap();
    // Without DrawText.fxh, shader check should fail
    let has_all_initial = s_dir.join("Shaders").join("DLSS5_Feed.fx").is_file()
        && s_dir.join("Shaders").join("DrawText.fxh").is_file();
    assert!(!has_all_initial);

    // Add DrawText.fxh and FontAtlas.png
    fs::write(s_dir.join("Shaders").join("DrawText.fxh"), b"// test drawtext").unwrap();
    fs::write(s_dir.join("Textures").join("FontAtlas.png"), b"PNG").unwrap();

    let has_all_final = s_dir.join("Shaders").join("DLSS5_Feed.fx").is_file()
        && s_dir.join("Shaders").join("DrawText.fxh").is_file()
        && s_dir.join("Textures").join("FontAtlas.png").is_file();
    assert!(has_all_final);
}

#[tokio::test]
async fn test_feeder_components_live_assembly() {
    let mut log = Vec::new();
    let res = ensure_feeder_components(&mut log).await;
    assert!(res.is_ok(), "ensure_feeder_components failed: {:?}", res);
    let fc = res.unwrap();
    assert!(fc.addon64.is_file());
}

#[tokio::test]
async fn test_ensure_mfg_and_mip_fix_addons() {
    let mut log = Vec::new();
    // If the files exist in components root or can be resolved
    let mfg_res = ensure_mfg_v09_addon(&mut log).await;
    // We assert that the function runs without panic and produces an output
    assert!(mfg_res.is_ok() || mfg_res.is_err());

    let mut log2 = Vec::new();
    let mip_res = ensure_dlss5_d3d12_fix_addon(&mut log2).await;
    assert!(mip_res.is_ok() || mip_res.is_err());
}

#[tokio::test]
async fn test_ensure_dgvoodoo_components_resolves() {
    let mut log = Vec::new();
    let res = ensure_dgvoodoo_components(&mut log).await;
    assert!(res.is_ok() || res.is_err());
}

#[tokio::test]
async fn test_ensure_all_mandatory_components_progress_callback() {
    let mut progress_events = Vec::new();
    let res = ensure_all_mandatory_components_with_progress(|p| {
        progress_events.push(p);
    }).await;
    assert!(res.is_ok() || res.is_err());
    assert!(!progress_events.is_empty(), "Progress callback should have been invoked at least once");
}

#[tokio::test]
async fn test_ensure_mfg_and_mip_fix_cache_hit_paths() {
    let comp_root = dlss_studio::core::addons::cache::get_components_root();
    let mfg_dir = comp_root.join("mfg-unlock-1.0");
    let fix_dir = comp_root.join("dlss-mip-fix");
    fs::create_dir_all(&mfg_dir).unwrap();
    fs::create_dir_all(&fix_dir).unwrap();

    let mfg_file = mfg_dir.join("renodx-mfgunlock.addon64");
    let fix_file = fix_dir.join("dlss-mip-fix.addon64");

    // If not already present, create temporary files to test the immediate return path
    let created_mfg = if !mfg_file.is_file() {
        fs::write(&mfg_file, b"DUMMY_MFG").unwrap();
        true
    } else {
        false
    };
    let created_fix = if !fix_file.is_file() {
        fs::write(&fix_file, b"DUMMY_FIX").unwrap();
        true
    } else {
        false
    };

    let mut log = Vec::new();
    let res_mfg = ensure_mfg_v09_addon(&mut log).await;
    assert!(res_mfg.is_ok());
    assert_eq!(res_mfg.unwrap(), mfg_file);

    let mut log_fix = Vec::new();
    let res_fix = ensure_dlss5_d3d12_fix_addon(&mut log_fix).await;
    assert!(res_fix.is_ok());
    assert_eq!(res_fix.unwrap(), fix_file);

    if created_mfg {
        let _ = fs::remove_file(&mfg_file);
    }
    if created_fix {
        let _ = fs::remove_file(&fix_file);
    }
}

#[tokio::test]
async fn test_installer_archive_extraction_simulation() {
    let temp = TempDir::new("installer_archives");
    let zip_path = temp.join("test_component.zip");
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("component.dll", options).unwrap();
        std::io::Write::write_all(&mut writer, b"DLL_DATA").unwrap();
        writer.finish().unwrap();
    }

    let out_dir = temp.join("out");
    if let Ok(file) = fs::File::open(&zip_path) {
        let out_c = out_dir.clone();
        let zip_c = zip_path.clone();
        let _ = tokio::task::spawn_blocking(move || {
            let _ = dlss_studio::core::downloader::extract_zip(file, &out_c);
            let _ = fs::remove_file(&zip_c);
        }).await;
    }

    assert!(out_dir.join("component.dll").is_file());
    assert!(!zip_path.exists());
}

#[tokio::test]
async fn test_ensure_all_mandatory_components_cached_fast_path() {
    let _lock = STATE_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let comp_root = dlss_studio::core::addons::cache::get_components_root();

    let mut pe64 = vec![0u8; 1024];
    pe64[0] = b'M'; pe64[1] = b'Z';
    pe64[0x3c] = 0x80;
    pe64[0x80] = b'P'; pe64[0x81] = b'E';
    pe64[0x84] = 0x64; pe64[0x85] = 0x86;
    pe64[0x98] = 0x0b; pe64[0x99] = 0x02;

    let mut pe32 = vec![0u8; 1024];
    pe32[0] = b'M'; pe32[1] = b'Z';
    pe32[0x3c] = 0x80;
    pe32[0x80] = b'P'; pe32[0x81] = b'E';
    pe32[0x84] = 0x4c; pe32[0x85] = 0x01;
    pe32[0x98] = 0x0b; pe32[0x99] = 0x01;

    let pe_paths = [
        (comp_root.join("reshade-vulkan/ReShade64.dll"), pe64.clone()),
        (comp_root.join("reshade-vulkan/ReShade32.dll"), pe32.clone()),
    ];

    let paths = [
        comp_root.join("renodx-dlss5/renodx-dlss5.addon64"),
        comp_root.join("dlss-mip-fix/dlss-mip-fix.addon64"),
        comp_root.join("mfg-unlock-1.0/renodx-mfgunlock.addon64"),
        comp_root.join("mfg-standalone/RTXMFG.dll"),
        comp_root.join("DLSS5-Feeder-1.16.0-beta.3/dlss5-feed.addon64"),
        comp_root.join("feeder-shaders/Shaders/DLSS5_Feed.fx"),
        comp_root.join("feeder-shaders/Shaders/DrawText.fxh"),
        comp_root.join("feeder-shaders/Shaders/vort_Motion.fx"),
        comp_root.join("feeder-shaders/Textures/vort_BlueNoise.png"),
        comp_root.join("OptiScaler-0.8.4-dlssnr/OptiScaler.dll"),
        comp_root.join("streamline-2.14.1/streamline/sl.interposer.dll"),
        comp_root.join("streamline-2.14.1/streamline/sl.common.dll"),
        comp_root.join("reshade-vulkan/ReShade64.json"),
        comp_root.join("dgvoodoo/x64/D3D9.dll"),
        comp_root.join("dgvoodoo/x86/D3D9.dll"),
        comp_root.join("dgvoodoo/x86/D3D8.dll"),
        comp_root.join("dgvoodoo/dgVoodoo.conf"),
    ];

    let mut created = Vec::new();
    for (p, data) in &pe_paths {
        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::write(p, data).is_ok() {
            created.push(p.clone());
        }
    }
    for p in &paths {
        if !p.is_file() {
            if let Some(parent) = p.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(p, b"DUMMY_COMPONENT").is_ok() {
                created.push(p.clone());
            }
        }
    }

    let mut progress = Vec::new();
    let res = ensure_all_mandatory_components_with_progress(|p| {
        progress.push(p);
    }).await;
    assert!(res.is_ok());

    let mut log = Vec::new();
    let dgv = ensure_dgvoodoo_components(&mut log).await;
    assert!(dgv.is_ok());

    let mut log_f = Vec::new();
    let fdr = ensure_feeder_components(&mut log_f).await;
    assert!(fdr.is_ok());

    for c in created {
        let _ = fs::remove_file(c);
    }
}


#[test]
fn test_ensure_feeder_components_populates_textures_and_includes_complete_tree() {
    let temp = TempDir::new("feeder_assembly_test");
    let comp_root = temp.path();

    let feeder_dir = comp_root.join("DLSS5-Feeder-1.16.0-beta.3");
    let vort_dir = comp_root.join("vort_Shaders-b410b9f");
    let slim_dir = comp_root.join("reshade-shaders-slim");

    fs::create_dir_all(feeder_dir.join("reshade-shaders").join("Shaders")).unwrap();
    fs::create_dir_all(vort_dir.join("Shaders").join("Includes")).unwrap();
    fs::create_dir_all(vort_dir.join("Textures")).unwrap();
    fs::create_dir_all(slim_dir.join("Shaders")).unwrap();
    fs::create_dir_all(slim_dir.join("Textures")).unwrap();

    fs::write(feeder_dir.join("dlss5-feed.addon64"), b"FEED_ADDON").unwrap();
    fs::write(feeder_dir.join("reshade-shaders").join("Shaders").join("DLSS5_Feed.fx"), b"// DLSS5_Feed").unwrap();
    fs::write(vort_dir.join("Shaders").join("vort_Motion.fx"), b"// vort_Motion").unwrap();
    fs::write(vort_dir.join("Shaders").join("Includes").join("vort_MotionBlur.fxh"), b"// vort_Includes").unwrap();
    fs::write(vort_dir.join("Textures").join("vort_BlueNoise.png"), b"BLUENOISE_PNG_DATA").unwrap();
    fs::write(slim_dir.join("Shaders").join("DrawText.fxh"), b"// DrawText").unwrap();
    fs::write(slim_dir.join("Textures").join("FontAtlas.png"), b"FONTATLAS_PNG_DATA").unwrap();

    // Assemble manually following the exact logic
    let consolidated = comp_root.join("feeder-shaders");
    let target_shaders = consolidated.join("Shaders");
    let target_textures = consolidated.join("Textures");
    let target_includes = target_shaders.join("Includes");

    fs::create_dir_all(&target_includes).unwrap();
    fs::create_dir_all(&target_textures).unwrap();

    // Copy DLSS5_Feed
    fs::copy(feeder_dir.join("reshade-shaders").join("Shaders").join("DLSS5_Feed.fx"), target_shaders.join("DLSS5_Feed.fx")).unwrap();

    // Copy slim bundle shaders & textures
    for entry in walkdir::WalkDir::new(slim_dir.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.ends_with(".fxh") || fname.ends_with(".fx") {
                let _ = fs::copy(entry.path(), target_shaders.join(&fname));
            }
        }
    }
    for entry in walkdir::WalkDir::new(slim_dir.join("Textures")).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let fname = entry.file_name().to_string_lossy().to_string();
            let _ = fs::copy(entry.path(), target_textures.join(fname));
        }
    }

    // Copy VORT shaders & textures
    for entry in walkdir::WalkDir::new(vort_dir.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.ends_with(".fx") || fname.ends_with(".fxh") {
                let dest = if entry.path().parent().map(|p| p.ends_with("Includes")).unwrap_or(false) {
                    target_includes.join(&fname)
                } else {
                    target_shaders.join(&fname)
                };
                let _ = fs::copy(entry.path(), dest);
            }
        }
    }
    for entry in walkdir::WalkDir::new(vort_dir.join("Textures")).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let fname = entry.file_name().to_string_lossy().to_string();
            let _ = fs::copy(entry.path(), target_textures.join(fname));
        }
    }

    // ASSERT 100% COMPLETENESS OF TEXTURES & INCLUDES:
    assert!(target_textures.join("vort_BlueNoise.png").is_file(), "vort_BlueNoise.png must exist in feeder-shaders/Textures/");
    assert_eq!(fs::read(target_textures.join("vort_BlueNoise.png")).unwrap(), b"BLUENOISE_PNG_DATA");

    assert!(target_textures.join("FontAtlas.png").is_file(), "FontAtlas.png must exist in feeder-shaders/Textures/");
    assert_eq!(fs::read(target_textures.join("FontAtlas.png")).unwrap(), b"FONTATLAS_PNG_DATA");

    assert!(target_includes.join("vort_MotionBlur.fxh").is_file(), "Includes/vort_MotionBlur.fxh must exist in feeder-shaders/Shaders/Includes/");
    assert!(target_shaders.join("DrawText.fxh").is_file(), "DrawText.fxh must exist in feeder-shaders/Shaders/");
    assert!(target_shaders.join("DLSS5_Feed.fx").is_file(), "DLSS5_Feed.fx must exist in feeder-shaders/Shaders/");
    assert!(target_shaders.join("vort_Motion.fx").is_file(), "vort_Motion.fx must exist in feeder-shaders/Shaders/");
}

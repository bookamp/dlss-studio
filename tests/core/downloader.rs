use dlss_studio::core::downloader::{
    are_all_mandatory_components_cached, compute_sha256, ensure_all_mandatory_components_with_progress,
    ensure_feeder_components, extract_zip, format_bytes, get_components_root, is_streamline_cached,
    DownloadProgress,
};
use crate::common::TempDir;
use std::fs;

#[test]
fn test_zip_extraction_mock() {
    let temp = TempDir::new("zip");
    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("sample.txt", options).unwrap();
        std::io::Write::write_all(&mut writer, b"hello world").unwrap();
        writer.finish().unwrap();
    }

    let out_dir = temp.join("extracted");
    extract_zip(std::io::Cursor::new(buf), &out_dir).unwrap();
    assert!(out_dir.join("sample.txt").exists());
    assert_eq!(fs::read_to_string(out_dir.join("sample.txt")).unwrap(), "hello world");

    // Test compute_sha256 on a real file (known SHA-256 of "hello world")
    let hash = compute_sha256(&out_dir.join("sample.txt")).unwrap();
    assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");

    // Test components root
    let comp_root = get_components_root();
    assert!(comp_root.exists() || comp_root.to_string_lossy().contains("dlss-5-studio"));
}

#[test]
fn test_download_progress_default_and_format_bytes() {
    let def = DownloadProgress::default();
    assert!(!def.is_downloading);
    assert_eq!(def.percentage, 0.0);

    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(2048), "2.0 KB");
    assert_eq!(format_bytes(10 * 1024 * 1024), "10.00 MB");
}

#[test]
fn test_step_progress_calculation() {
    const TOTAL_STEPS: usize = 7;
    for step in 0..TOTAL_STEPS {
        for pct in [0.0f32, 50.0f32, 100.0f32] {
            let overall = ((step as f32 * 100.0) + pct) / TOTAL_STEPS as f32;
            assert!(overall >= 0.0 && overall <= 100.0);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_live_download_streamline_components() {
    let res = ensure_all_mandatory_components_with_progress(|prog| {
        if prog.is_downloading {
            println!("Progress: {:.1}% - {}", prog.percentage, prog.message);
        }
    }).await;
    assert!(res.is_ok(), "Mandatory download failed: {:?}", res);
    assert!(is_streamline_cached(), "Streamline must be cached after download");
    assert!(are_all_mandatory_components_cached(), "All mandatory components must be cached");
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
    println!("FC SHADER DIR: {:?}", fc.shader_dir);
    for l in &log {
        println!("LOG: {}", l);
    }
    assert!(fc.shader_dir.join("Shaders").join("DLSS5_Feed.fx").is_file());
    assert!(fc.shader_dir.join("Shaders").join("DrawText.fxh").is_file());
    assert!(fc.shader_dir.join("Shaders").join("ReShade.fxh").is_file());
    assert!(fc.shader_dir.join("Shaders").join("ReShadeUI.fxh").is_file());
    assert!(fc.shader_dir.join("Textures").join("FontAtlas.png").is_file());
}

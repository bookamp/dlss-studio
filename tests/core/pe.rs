use dlss_studio::core::pe::{
    inspect_pe, is_large_address_aware, is_legacy_streamline_1x, is_optiscaler_or_proxy,
    is_reshade_dll, set_large_address_aware,
};
use crate::common::TempDir;
use std::path::Path;

#[test]
fn test_is_optiscaler_or_proxy_with_marker() {
    let temp_dir = TempDir::new("pe_test");
    let dummy_dll = temp_dir.path().join("dxgi.dll");

    // Plain binary without markers
    std::fs::write(&dummy_dll, vec![0u8; 1024]).unwrap();
    assert!(!is_optiscaler_or_proxy(&dummy_dll));

    // Binary with OptiScaler marker
    let mut with_marker = vec![0u8; 100_000];
    with_marker[50_000..50_010].copy_from_slice(b"OptiScaler");
    std::fs::write(&dummy_dll, with_marker).unwrap();
    assert!(is_optiscaler_or_proxy(&dummy_dll));

    // Binary with Streamline marker
    let mut with_streamline = vec![0u8; 10_000];
    with_streamline[1000..1010].copy_from_slice(b"Streamline");
    std::fs::write(&dummy_dll, with_streamline).unwrap();
    assert!(is_optiscaler_or_proxy(&dummy_dll));

    // Missing file
    assert!(!is_optiscaler_or_proxy(temp_dir.path().join("missing.dll")));
    assert!(inspect_pe(temp_dir.path().join("missing.dll")).is_none());

    let (mentions, ver, addon) = is_reshade_dll(&dummy_dll);
    assert!(!mentions);
    assert!(ver.is_none());
    assert!(!addon);

    let reshade_payload = Path::new("payload/reshade-vulkan/ReShade64.dll");
    let reshade_cached = dlss_studio::core::downloader::get_components_root().join("reshade-vulkan/ReShade64.dll");
    let target = if reshade_payload.is_file() { Some(reshade_payload) } else if reshade_cached.is_file() { Some(reshade_cached.as_path()) } else { None };
    if let Some(target_p) = target {
        let (is_res, _, has_add) = is_reshade_dll(target_p);
        assert!(is_res);
        assert!(has_add);
    }
}

#[test]
fn test_large_address_aware_inspection_and_toggle() {
    let temp_dir = TempDir::new("pe_laa");
    let exe_path = temp_dir.path().join("TestGame32.exe");

    // Construct minimal valid PE32 with Characteristics = 0x0103 (LAA = false)
    let mut data = vec![0u8; 512];
    data[0] = b'M';
    data[1] = b'Z';
    let pe_offset: u32 = 0x80;
    data[0x3C..0x40].copy_from_slice(&pe_offset.to_le_bytes());

    // PE signature
    data[0x80..0x84].copy_from_slice(b"PE\0\0");
    // Machine = i386 (0x014C)
    data[0x84..0x86].copy_from_slice(&0x014Cu16.to_le_bytes());
    // NumberOfSections = 1
    data[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    // SizeOfOptionalHeader = 0xE0
    data[0x94..0x96].copy_from_slice(&0x00E0u16.to_le_bytes());
    // Characteristics = 0x0103 (IMAGE_FILE_32BIT_MACHINE | IMAGE_FILE_EXECUTABLE_IMAGE | IMAGE_FILE_RELOCS_STRIPPED)
    data[0x96..0x98].copy_from_slice(&0x0103u16.to_le_bytes());
    // Optional header magic = 0x010B (PE32)
    data[0x98..0x9A].copy_from_slice(&0x010Bu16.to_le_bytes());

    std::fs::write(&exe_path, &data).unwrap();

    assert!(!is_large_address_aware(&exe_path), "Initial binary must not be LAA");
    let pe_info = inspect_pe(&exe_path).expect("Must inspect synthetic PE");
    assert_eq!(pe_info.bitness, 32);
    assert!(!pe_info.is_laa, "PeInfo must report is_laa = false");

    // Enable LAA
    let changed = set_large_address_aware(&exe_path, true).expect("set_large_address_aware(true) must succeed");
    assert!(changed, "Must report change made");
    assert!(is_large_address_aware(&exe_path), "Binary must now be LAA");

    let pe_info_after = inspect_pe(&exe_path).expect("Must inspect synthetic PE after LAA patch");
    assert!(pe_info_after.is_laa, "PeInfo must report is_laa = true after patch");

    // Re-enabling when already enabled should be a no-op
    let changed2 = set_large_address_aware(&exe_path, true).expect("Re-enabling must succeed");
    assert!(!changed2, "Should return false if already enabled");

    // Disable LAA
    let changed3 = set_large_address_aware(&exe_path, false).expect("Disabling must succeed");
    assert!(changed3);
    assert!(!is_large_address_aware(&exe_path));
}

#[test]
fn test_is_legacy_streamline_1x_detects_legacy_exports() {
    let temp_dir = TempDir::new("pe_sl1");
    let sl_file = temp_dir.path().join("sl.interposer.dll");

    let mut bytes = vec![0u8; 4096];
    bytes[100..121].copy_from_slice(b"slGetFeatureSettings\0");
    std::fs::write(&sl_file, bytes).unwrap();

    assert!(is_legacy_streamline_1x(&sl_file), "Must detect legacy Streamline 1.x with slGetFeatureSettings");
}

#[test]
fn test_is_legacy_streamline_1x_rejects_modern_streamline() {
    let temp_dir = TempDir::new("pe_sl2");
    let sl_file = temp_dir.path().join("sl.interposer.dll");

    let mut bytes = vec![0u8; 4096];
    bytes[100..107].copy_from_slice(b"slInit\0");
    bytes[200..218].copy_from_slice(b"slEvaluateFeature\0");
    std::fs::write(&sl_file, bytes).unwrap();

    assert!(!is_legacy_streamline_1x(&sl_file), "Modern Streamline 2.x without legacy exports must NOT be marked legacy");
}

use dlss_studio::core::pe::{
    inspect_pe, is_large_address_aware, is_legacy_streamline_1x, is_optiscaler_or_proxy,
    is_reshade_dll, set_large_address_aware,
};
use crate::common::TempDir;

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

    // Minimal valid PE with ReShade marker
    let mut reshade_buf = vec![0u8; 4096];
    reshade_buf[0] = b'M';
    reshade_buf[1] = b'Z';
    let pe_offset: u32 = 0x80;
    reshade_buf[0x3C..0x40].copy_from_slice(&pe_offset.to_le_bytes());
    reshade_buf[0x80..0x84].copy_from_slice(b"PE\0\0");
    reshade_buf[0x84..0x86].copy_from_slice(&0x014Cu16.to_le_bytes());
    reshade_buf[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    reshade_buf[0x94..0x96].copy_from_slice(&0x00E0u16.to_le_bytes());
    reshade_buf[0x96..0x98].copy_from_slice(&0x0103u16.to_le_bytes());
    reshade_buf[0x98..0x9A].copy_from_slice(&0x010Bu16.to_le_bytes());
    reshade_buf[500..522].copy_from_slice(b"Searching for add-ons\0");
    reshade_buf[600..608].copy_from_slice(b"ReShade\0");
    let reshade_synthetic = temp_dir.path().join("ReShadeSynthetic.dll");
    std::fs::write(&reshade_synthetic, reshade_buf).unwrap();
    let (is_res, _, has_add) = is_reshade_dll(&reshade_synthetic);
    assert!(is_res);
    assert!(has_add);
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

#[test]
fn test_is_legacy_streamline_1x_additional_signatures_and_missing() {
    let temp_dir = TempDir::new("pe_sl_add");
    assert!(!is_legacy_streamline_1x(temp_dir.path().join("missing.dll")));

    // slGetHooks signature
    let sl_file1 = temp_dir.path().join("sl1.dll");
    let mut bytes1 = vec![0u8; 2048];
    bytes1[50..61].copy_from_slice(b"slGetHooks\0");
    std::fs::write(&sl_file1, bytes1).unwrap();
    assert!(is_legacy_streamline_1x(&sl_file1));

    // slGetFeatureConfiguration signature
    let sl_file2 = temp_dir.path().join("sl2.dll");
    let mut bytes2 = vec![0u8; 2048];
    bytes2[50..76].copy_from_slice(b"slGetFeatureConfiguration\0");
    std::fs::write(&sl_file2, bytes2).unwrap();
    assert!(is_legacy_streamline_1x(&sl_file2));
}

#[test]
fn test_set_large_address_aware_error_paths() {
    let temp_dir = TempDir::new("laa_err");
    // Missing file
    assert!(set_large_address_aware(temp_dir.path().join("missing.exe"), true).is_err());
    // Not MZ
    let not_mz = temp_dir.path().join("not_mz.exe");
    std::fs::write(&not_mz, b"NOT_MZ_HEADER").unwrap();
    assert!(set_large_address_aware(&not_mz, true).is_err());
    assert!(!is_large_address_aware(&not_mz));
    // Short file
    let short_file = temp_dir.path().join("short.exe");
    std::fs::write(&short_file, b"MZ").unwrap();
    assert!(set_large_address_aware(&short_file, true).is_err());
    assert!(!is_large_address_aware(&short_file));
}

#[test]
fn test_is_reshade_dll_with_synthetic_markers() {
    let temp_dir = TempDir::new("pe_reshade");
    let dll_path = temp_dir.path().join("reshade_mock.dll");

    // Construct valid PE32 with ReShade marker and addon marker
    let mut data = vec![0u8; 4096];
    data[0] = b'M';
    data[1] = b'Z';
    let pe_offset: u32 = 0x80;
    data[0x3C..0x40].copy_from_slice(&pe_offset.to_le_bytes());
    data[0x80..0x84].copy_from_slice(b"PE\0\0");
    data[0x84..0x86].copy_from_slice(&0x014Cu16.to_le_bytes());
    data[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    data[0x94..0x96].copy_from_slice(&0x00E0u16.to_le_bytes());
    data[0x96..0x98].copy_from_slice(&0x0103u16.to_le_bytes());
    data[0x98..0x9A].copy_from_slice(&0x010Bu16.to_le_bytes());

    // Inject "Searching for add-ons" and "ReShade"
    data[500..522].copy_from_slice(b"Searching for add-ons\0");
    data[600..608].copy_from_slice(b"ReShade\0");

    std::fs::write(&dll_path, &data).unwrap();

    let (is_res, _, has_addon) = is_reshade_dll(&dll_path);
    assert!(is_res);
    assert!(has_addon);
}

#[test]
fn test_inspect_synthetic_pe64() {
    let temp_dir = TempDir::new("pe64_test");
    let exe_path = temp_dir.path().join("TestGame64.exe");

    let mut data = vec![0u8; 1024];
    data[0] = b'M';
    data[1] = b'Z';
    let pe_offset: u32 = 0x80;
    data[0x3C..0x40].copy_from_slice(&pe_offset.to_le_bytes());

    data[0x80..0x84].copy_from_slice(b"PE\0\0");
    // Machine = AMD64 (0x8664)
    data[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
    data[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    data[0x94..0x96].copy_from_slice(&0x00F0u16.to_le_bytes());
    data[0x96..0x98].copy_from_slice(&0x0022u16.to_le_bytes());
    // Optional header magic = 0x020B (PE32+)
    data[0x98..0x9A].copy_from_slice(&0x020Bu16.to_le_bytes());

    std::fs::write(&exe_path, &data).unwrap();

    let info = inspect_pe(&exe_path).expect("Must inspect PE64");
    assert_eq!(info.bitness, 64);
    assert!(info.is_laa);
}

#[test]
fn test_is_optiscaler_markers_variety() {
    let temp_dir = TempDir::new("opti_markers");
    let markers = ["DLSS-NR", "RTXMFG", "Dashdogy", "dgVoodoo"];

    for m in &markers {
        let p = temp_dir.path().join(format!("{}.dll", m));
        let mut bytes = vec![0u8; 2048];
        bytes[100..100 + m.len()].copy_from_slice(m.as_bytes());
        std::fs::write(&p, bytes).unwrap();
        assert!(is_optiscaler_or_proxy(&p), "Marker {} should be detected", m);
    }
}


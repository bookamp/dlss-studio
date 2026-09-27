use dlss_studio::core::tray::{
    fill_u16_buf, is_startup_enabled, request_show_window, set_startup_enabled,
    take_show_window_request, to_wide,
};

#[test]
fn test_startup_registry_read() {
    let _ = is_startup_enabled();
}

#[test]
fn test_to_wide_null_termination() {
    let wide = to_wide("DLSS Studio");
    assert_eq!(*wide.last().unwrap(), 0);
    assert_eq!(wide.len(), 12);
}

#[test]
fn test_fill_u16_buf_padding() {
    let mut buf = [0u16; 16];
    fill_u16_buf(&mut buf, "Test");
    assert_eq!(buf[0], 'T' as u16);
    assert_eq!(buf[1], 'e' as u16);
    assert_eq!(buf[2], 's' as u16);
    assert_eq!(buf[3], 't' as u16);
    assert_eq!(buf[4], 0);

    // Overflow truncation test
    let mut small_buf = [0u16; 4];
    fill_u16_buf(&mut small_buf, "TestingOverflow");
    assert_eq!(small_buf[3], 0);
}

#[test]
fn test_show_window_request_flag_lifecycle() {
    assert!(!take_show_window_request());
    request_show_window();
    assert!(take_show_window_request());
    assert!(!take_show_window_request());
}

#[test]
fn test_set_startup_enabled_roundtrip() {
    let initial = is_startup_enabled();
    let res = set_startup_enabled(initial);
    if res.is_ok() {
        assert_eq!(is_startup_enabled(), initial);
    }
}

#[test]
fn test_zero_python_files_in_repository() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut py_files = Vec::new();
    for entry in walkdir::WalkDir::new(&manifest_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.components().any(|c| c.as_os_str() == "target") {
            continue;
        }
        if p.extension().map_or(false, |ext| ext == "py") {
            py_files.push(p.strip_prefix(&manifest_dir).unwrap_or(p).to_path_buf());
        }
    }
    assert!(
        py_files.is_empty(),
        "Found Python files in repository! DLSS Studio must be 100% pure Rust: {:?}",
        py_files
    );
}

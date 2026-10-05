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
    let _ = take_show_window_request();
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
fn test_show_big_picture_request_flag_lifecycle() {
    assert!(!dlss_studio::core::tray::take_show_big_picture_request());
    dlss_studio::core::tray::request_show_big_picture();
    assert!(dlss_studio::core::tray::take_show_big_picture_request());
    assert!(!dlss_studio::core::tray::take_show_big_picture_request());
}

#[test]
fn test_trim_working_set_runs_safely() {
    dlss_studio::core::tray::trim_working_set();
}

#[test]
fn test_tray_update_helpers_safe_without_hwnd() {
    dlss_studio::core::tray::show_background_notification();
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

#[test]
fn test_tray_wnd_proc_messages() {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use dlss_studio::core::tray::{
        tray_wnd_proc, take_show_window_request, take_show_big_picture_request,
        WM_SHOW_WINDOW, WM_SHOW_BIG_PICTURE,
    };

    let dummy_hwnd = HWND(std::ptr::null_mut());

    // 1. WM_SHOW_WINDOW with wparam=0 (desktop window)
    take_show_window_request();
    take_show_big_picture_request();
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_SHOW_WINDOW, WPARAM(0), LPARAM(0));
    }
    assert!(take_show_window_request());
    assert!(!take_show_big_picture_request());

    // 2. WM_SHOW_WINDOW with wparam=1 (big picture window)
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_SHOW_WINDOW, WPARAM(1), LPARAM(0));
    }
    assert!(take_show_window_request());
    assert!(take_show_big_picture_request());

    // 3. WM_SHOW_BIG_PICTURE
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_SHOW_BIG_PICTURE, WPARAM(0), LPARAM(0));
    }
    assert!(take_show_window_request());
    assert!(take_show_big_picture_request());

    // 4. WM_TRAY_CALLBACK with WM_LBUTTONUP
    const WM_APP: u32 = 0x8000;
    const WM_TRAY_CALLBACK: u32 = WM_APP + 101;
    const WM_LBUTTONUP: isize = 0x0202;
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_TRAY_CALLBACK, WPARAM(0), LPARAM(WM_LBUTTONUP));
    }
    assert!(take_show_window_request());

    // 5. WM_TRAY_CALLBACK with WM_LBUTTONDBLCLK
    const WM_LBUTTONDBLCLK: isize = 0x0203;
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_TRAY_CALLBACK, WPARAM(0), LPARAM(WM_LBUTTONDBLCLK));
    }
    assert!(take_show_window_request());

    // 6. WM_TRAY_CALLBACK with WM_RBUTTONUP (triggers show_tray_context_menu)
    const WM_RBUTTONUP: isize = 0x0205;
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_TRAY_CALLBACK, WPARAM(0), LPARAM(WM_RBUTTONUP));
    }

    // 7. WM_TRAY_CALLBACK with WM_CONTEXTMENU
    const WM_CONTEXTMENU: isize = 0x007B;
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_TRAY_CALLBACK, WPARAM(0), LPARAM(WM_CONTEXTMENU));
    }

    // 8. WM_DESTROY
    const WM_DESTROY: u32 = 0x0002;
    unsafe {
        tray_wnd_proc(dummy_hwnd, WM_DESTROY, WPARAM(0), LPARAM(0));
    }

    // 9. Default message
    unsafe {
        tray_wnd_proc(dummy_hwnd, 0x1234, WPARAM(0), LPARAM(0));
    }
}

#[test]
fn test_system_tray_and_startup_toggle_lifecycles() {
    use dlss_studio::core::tray::{start_system_tray, show_background_notification};

    // 1. Startup registry toggle both true and false
    let was_enabled = is_startup_enabled();
    let _ = set_startup_enabled(true);
    let _ = set_startup_enabled(false);
    let _ = set_startup_enabled(was_enabled);

    // 2. Start system tray thread and let it pump messages
    start_system_tray();
    std::thread::sleep(std::time::Duration::from_millis(150));

    // 3. Show background notification with active TRAY_HWND
    show_background_notification();

    // 4. Signal tray to destroy itself cleanly
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_DESTROY};
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        let class_name = windows::core::w!("DLSS5StudioTrayClass");
        let window_name = windows::core::w!("DLSS5StudioTrayWindow");
        if let Ok(hwnd) = FindWindowW(class_name, window_name) {
            if !hwnd.0.is_null() {
                let _ = PostMessageW(hwnd, WM_DESTROY, WPARAM(0), LPARAM(0));
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
}


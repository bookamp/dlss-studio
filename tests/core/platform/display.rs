use dlss_studio::core::display::*;

#[test]
fn test_enumerate_monitors_runs() {
    let monitors = enumerate_monitors();
    #[cfg(windows)]
    {
        assert!(!monitors.is_empty(), "Should detect at least one display monitor");
    }
    let _ = monitors;
}

#[test]
fn test_find_streaming_display_fallback() {
    let display = find_streaming_display();
    #[cfg(windows)]
    {
        assert!(display.is_some(), "Should resolve a streaming display target");
    }
    let _ = display;
}

#[test]
fn test_desktop_window_default_size_constants() {
    assert_eq!(DEFAULT_DESKTOP_WIDTH, 1280);
    assert_eq!(DEFAULT_DESKTOP_HEIGHT, 900);
}

#[test]
fn test_is_virtual_display_present_runs() {
    let _ = is_virtual_display_present();
}

#[test]
fn test_is_streaming_display_active_runs() {
    let _ = is_streaming_display_active(None);
    let _ = is_streaming_display_active(Some("Nonexistent Display"));
}

#[test]
fn test_is_streaming_display_active_with_sunshine_env() {
    // Test with Sunshine client environment variables
    unsafe {
        std::env::set_var("SUNSHINE_CLIENT_WIDTH", "1920");
    }
    assert!(is_streaming_display_active(None), "Should report active streaming when Sunshine env is present");
    unsafe {
        std::env::remove_var("SUNSHINE_CLIENT_WIDTH");
    }
}

#[test]
fn test_restore_desktop_window_geometry_safe_execution() {
    // Safe to call even during unit testing when app hwnd might or might not be present
    let _ = restore_desktop_window_geometry();
}

#[test]
fn test_display_helpers_execution() {
    let _ = snap_window_to_streaming_display();
    let _ = ensure_window_not_stranded();
    #[cfg(windows)]
    {
        let _ = get_app_window_hwnd();
    }
}

#[test]
fn test_find_streaming_display_with_env_resolution() {
    unsafe {
        std::env::set_var("SUNSHINE_CLIENT_WIDTH", "3840");
        std::env::set_var("SUNSHINE_CLIENT_HEIGHT", "2160");
    }
    let res = find_streaming_display();
    #[cfg(windows)]
    {
        assert!(res.is_some());
    }
    let _ = res;
    unsafe {
        std::env::remove_var("SUNSHINE_CLIENT_WIDTH");
        std::env::remove_var("SUNSHINE_CLIENT_HEIGHT");
    }

    // Dynamic resolution test matching active monitor
    let monitors = enumerate_monitors();
    if let Some(first) = monitors.first() {
        unsafe {
            std::env::set_var("SUNSHINE_CLIENT_WIDTH", first.width.to_string());
            std::env::set_var("SUNSHINE_CLIENT_HEIGHT", first.height.to_string());
        }
        let matched = find_streaming_display();
        assert!(matched.is_some());
        let m = matched.unwrap();
        assert!(m.width > 0 && m.height > 0);
        unsafe {
            std::env::remove_var("SUNSHINE_CLIENT_WIDTH");
            std::env::remove_var("SUNSHINE_CLIENT_HEIGHT");
        }

        // Test is_streaming_display_active with device name
        assert!(is_streaming_display_active(Some(&first.device_name)));
    }
}

#[test]
fn test_monitor_rect_geometry_and_overlap_math() {
    let m = MonitorRect {
        left: 0,
        top: 0,
        width: 1920,
        height: 1080,
        is_primary: true,
        is_virtual: false,
        device_name: "\\\\.\\DISPLAY1".to_string(),
        description: "Primary Display".to_string(),
    };
    assert_eq!(m.left, 0);
    assert_eq!(m.width, 1920);
    assert!(m.is_primary);

    // Centering calculation check
    let target_x = m.left + ((m.width - DEFAULT_DESKTOP_WIDTH) / 2).max(0);
    let target_y = m.top + ((m.height - DEFAULT_DESKTOP_HEIGHT) / 2).max(0);
    assert_eq!(target_x, (1920 - 1280) / 2);
    assert_eq!(target_y, (1080 - 900) / 2);

    // Overlap checks
    let inside_l = 100;
    let inside_r = 1380;
    let inside_t = 100;
    let inside_b = 1000;
    let overlaps = inside_l < (m.left + m.width)
        && inside_r > m.left
        && inside_t < (m.top + m.height)
        && inside_b > m.top;
    assert!(overlaps);

    // Completely stranded to the right
    let stranded_l = 2500;
    let stranded_r = 3780;
    let stranded = stranded_l < (m.left + m.width)
        && stranded_r > m.left
        && inside_t < (m.top + m.height)
        && inside_b > m.top;
    assert!(!stranded);
}

#[cfg(windows)]
#[test]
fn test_get_app_window_hwnd_and_window_movement_with_real_window() {
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DefWindowProcW, RegisterClassW,
        WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };
    use windows::Win32::Foundation::{HWND, HINSTANCE};
    use windows::core::w;
    unsafe {
        unsafe extern "system" fn test_wndproc(
            hwnd: HWND,
            msg: u32,
            wparam: windows::Win32::Foundation::WPARAM,
            lparam: windows::Win32::Foundation::LPARAM,
        ) -> windows::Win32::Foundation::LRESULT {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        let class_name = w!("DLSS_TEST_WIN_CLASS");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(test_wndproc);
        wc.hInstance = HINSTANCE::default();
        wc.lpszClassName = class_name;
        let _ = RegisterClassW(&wc);

        let title = w!("DLSS 5 Studio - Test Window");
        let hwnd_res = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            class_name,
            title,
            WS_OVERLAPPEDWINDOW,
            100, 100, 800, 600,
            HWND::default(),
            windows::Win32::UI::WindowsAndMessaging::HMENU::default(),
            HINSTANCE::default(),
            None,
        );

        if let Ok(hwnd) = hwnd_res {
            if !hwnd.is_invalid() {
                // Now get_app_window_hwnd will locate THIS window for the current process!
                let found = get_app_window_hwnd();
                assert!(found.is_some());
                assert_eq!(found.unwrap(), hwnd);

                // Now snap_window_to_streaming_display executes ShowWindow, SetWindowPos, etc.
                let snapped = snap_window_to_streaming_display();
                assert!(snapped);

                // Now restore_desktop_window_geometry executes ShowWindow, SetWindowPos, etc.
                let restored = restore_desktop_window_geometry();
                assert!(restored);

                // Now ensure_window_not_stranded executes GetWindowRect
                let _ = ensure_window_not_stranded();

                let _ = DestroyWindow(hwnd);
            }
        }
    }
}


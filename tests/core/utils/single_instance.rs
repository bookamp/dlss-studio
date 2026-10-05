use dlss_studio::core::single_instance::{acquire_named_mutex, signal_existing_instance};

#[test]
fn test_single_instance_guard_lifecycle() {
    let test_mutex = windows::core::w!("Local\\DLSS5_Studio_Test_Mutex_9999");
    let guard = acquire_named_mutex(test_mutex);
    assert!(guard.is_some(), "First acquisition in test must succeed");
    let second = acquire_named_mutex(test_mutex);
    assert!(second.is_none(), "Second acquisition while guard is held must be None");
    drop(guard);
    let third = acquire_named_mutex(test_mutex);
    assert!(third.is_some(), "Acquisition after dropping guard must succeed");
}

#[test]
fn test_signal_existing_instance_call() {
    signal_existing_instance(false);
    signal_existing_instance(true);

    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, DefWindowProcW, RegisterClassW,
            WNDCLASSW, WS_OVERLAPPEDWINDOW,
        };
        use windows::Win32::Foundation::{HWND, HINSTANCE};
        use windows::core::w;

        unsafe extern "system" fn test_wndproc(
            hwnd: HWND,
            msg: u32,
            wparam: windows::Win32::Foundation::WPARAM,
            lparam: windows::Win32::Foundation::LPARAM,
        ) -> windows::Win32::Foundation::LRESULT {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        let class_name = w!("DLSS5StudioTrayClass");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(test_wndproc);
        wc.hInstance = HINSTANCE::default();
        wc.lpszClassName = class_name;
        let _ = RegisterClassW(&wc);

        let window_name = w!("DLSS5StudioTrayWindow");
        if let Ok(hwnd) = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            class_name,
            window_name,
            WS_OVERLAPPEDWINDOW,
            0, 0, 100, 100,
            HWND::default(),
            windows::Win32::UI::WindowsAndMessaging::HMENU::default(),
            HINSTANCE::default(),
            None,
        ) {
            signal_existing_instance(false);
            signal_existing_instance(true);
            let _ = DestroyWindow(hwnd);
        }
    }

    // Hold the single instance mutex so acquire_single_instance hits the secondary branch
    let primary_mutex = windows::core::w!("Local\\DLSS5_Studio_SingleInstance_Mutex");
    let guard = acquire_named_mutex(primary_mutex);
    if guard.is_some() {
        let second_desktop = dlss_studio::core::single_instance::acquire_single_instance(false);
        assert!(second_desktop.is_none());
        let second_bp = dlss_studio::core::single_instance::acquire_single_instance(true);
        assert!(second_bp.is_none());
        drop(guard);
    }
}


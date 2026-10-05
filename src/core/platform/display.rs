//! Monitor and Display helper module for DLSS Studio.
//! Detects active streaming monitors (IddSampleDriver / Sunshine / Apollo virtual displays)
//! and snaps the application window to the target display for seamless Moonlight streaming.

#[derive(Debug, Clone)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub is_primary: bool,
    pub is_virtual: bool,
    pub device_name: String,
    pub description: String,
}

#[cfg(windows)]
pub fn enumerate_monitors() -> Vec<MonitorRect> {
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW, HDC, HMONITOR,
        MONITORINFOEXW,
    };

    let mut monitors = Vec::new();

    unsafe extern "system" fn monitor_enum_proc(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors_ptr = lparam.0 as *mut Vec<MonitorRect>;
        let mut mi: MONITORINFOEXW = std::mem::zeroed();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmon, &mut mi as *mut _ as *mut _).as_bool() {
            let left = mi.monitorInfo.rcMonitor.left;
            let top = mi.monitorInfo.rcMonitor.top;
            let width = mi.monitorInfo.rcMonitor.right - mi.monitorInfo.rcMonitor.left;
            let height = mi.monitorInfo.rcMonitor.bottom - mi.monitorInfo.rcMonitor.top;
            let is_primary = (mi.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY

            let dev_name_len = mi
                .szDevice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(mi.szDevice.len());
            let dev_name = String::from_utf16_lossy(&mi.szDevice[..dev_name_len]);

            // Query display device info for hardware ID and description
            let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
            dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

            let mut desc = dev_name.clone();
            let mut dev_id = String::new();

            if EnumDisplayDevicesW(
                windows::core::PCWSTR::from_raw(mi.szDevice.as_ptr()),
                0,
                &mut dd,
                0,
            )
            .as_bool()
            {
                let str_len = dd
                    .DeviceString
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(dd.DeviceString.len());
                desc = String::from_utf16_lossy(&dd.DeviceString[..str_len]);

                let id_len = dd
                    .DeviceID
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(dd.DeviceID.len());
                dev_id = String::from_utf16_lossy(&dd.DeviceID[..id_len]);
            }

            let upper_desc = desc.to_uppercase();
            let upper_id = dev_id.to_uppercase();
            let upper_name = dev_name.to_uppercase();

            let is_virtual = upper_id.contains("SDD")
                || upper_id.contains("VIRTUAL")
                || upper_id.contains("IDD")
                || upper_desc.contains("VIRTUAL")
                || upper_desc.contains("SUNSHINE")
                || upper_desc.contains("APOLLO")
                || upper_desc.contains("VIBE")
                || upper_name.contains("VIRTUAL");

            let rect = MonitorRect {
                left,
                top,
                width,
                height,
                is_primary,
                is_virtual,
                device_name: dev_name,
                description: desc,
            };

            (*monitors_ptr).push(rect);
        }

        BOOL(1)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }

    monitors
}

#[cfg(not(windows))]
pub fn enumerate_monitors() -> Vec<MonitorRect> {
    Vec::new()
}

/// Identifies the best target display for Big Picture / Moonlight streaming:
/// 1. Any virtual display adapter (Sunshine/Apollo/IddSampleDriver, SDD)
/// 2. Monitor matching SUNSHINE_CLIENT_WIDTH x SUNSHINE_CLIENT_HEIGHT if present
/// 3. Primary monitor (often set to the streaming display by Sunshine ensure_primary)
/// 4. Fallback to first available monitor
pub fn find_streaming_display() -> Option<MonitorRect> {
    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        return None;
    }

    // 1. Any detected virtual display driver monitor
    if let Some(m) = monitors.iter().find(|m| m.is_virtual) {
        return Some(m.clone());
    }

    // 2. Matching Sunshine client resolution if environment variables are set
    if let (Ok(w_str), Ok(h_str)) = (
        std::env::var("SUNSHINE_CLIENT_WIDTH"),
        std::env::var("SUNSHINE_CLIENT_HEIGHT"),
    ) {
        if let (Ok(w), Ok(h)) = (w_str.parse::<i32>(), h_str.parse::<i32>()) {
            if let Some(m) = monitors.iter().find(|m| m.width == w && m.height == h) {
                return Some(m.clone());
            }
        }
    }

    // 3. Primary monitor
    if let Some(m) = monitors.iter().find(|m| m.is_primary) {
        return Some(m.clone());
    }

    // 4. Default to first monitor
    monitors.first().cloned()
}

#[cfg(windows)]
pub fn get_app_window_hwnd() -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId,
    };

    let my_pid = unsafe { GetCurrentProcessId() };
    let found: Option<HWND> = None;

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ptr = lparam.0 as *mut (u32, Option<HWND>);
        let (target_pid, ref mut res) = *ptr;

        let mut win_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut win_pid));

        if win_pid == target_pid {
            let mut title_buf: [u16; 64] = [0; 64];
            let len = GetWindowTextW(hwnd, &mut title_buf);
            if len > 0 {
                let title = String::from_utf16_lossy(&title_buf[..len as usize]);
                // Main window has title "DLSS 5 Studio" (ignore message-only windows or sub-windows)
                if title.contains("DLSS") {
                    *res = Some(hwnd);
                    return BOOL(0); // Stop enumeration
                }
            }
        }

        BOOL(1)
    }

    let mut state = (my_pid, found);
    unsafe {
        let _ = EnumWindows(Some(enum_callback), LPARAM(&mut state as *mut _ as isize));
    }

    state.1
}

/// Moves and snaps the application window to the target streaming/virtual display.
pub fn snap_window_to_streaming_display() -> bool {
    let target = match find_streaming_display() {
        Some(m) => m,
        None => return false,
    };

    crate::core::logger::info(
        "display",
        &format!(
            "Snapping window to streaming display: {} [{}] ({}x{} at {},{}, virtual: {}, primary: {})",
            target.description,
            target.device_name,
            target.width,
            target.height,
            target.left,
            target.top,
            target.is_virtual,
            target.is_primary
        ),
    );

    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOP, SWP_FRAMECHANGED,
            SWP_SHOWWINDOW, SW_RESTORE,
        };

        if let Some(hwnd) = get_app_window_hwnd() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOP,
                    target.left,
                    target.top,
                    target.width,
                    target.height,
                    SWP_SHOWWINDOW | SWP_FRAMECHANGED,
                );
                let _ = SetForegroundWindow(hwnd);
            }
            return true;
        }
    }

    false
}

pub const DEFAULT_DESKTOP_WIDTH: i32 = 1280;
pub const DEFAULT_DESKTOP_HEIGHT: i32 = 900;

/// Restores the application window geometry to the standard centered desktop dimensions (1280x900).
pub fn restore_desktop_window_geometry() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetForegroundWindow, SetWindowPos, ShowWindow, HWND_NOTOPMOST, SWP_FRAMECHANGED,
            SWP_SHOWWINDOW, SW_RESTORE, LoadCursorW, SetCursor, IDC_ARROW,
        };
        use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;

        if let Some(hwnd) = get_app_window_hwnd() {
            let monitors = enumerate_monitors();
            let primary = monitors.iter().find(|m| m.is_primary).or_else(|| monitors.first());

            let (target_x, target_y) = if let Some(m) = primary {
                let x = m.left + ((m.width - DEFAULT_DESKTOP_WIDTH) / 2).max(0);
                let y = m.top + ((m.height - DEFAULT_DESKTOP_HEIGHT) / 2).max(0);
                (x, y)
            } else {
                (100, 100)
            };

            crate::core::logger::info(
                "display",
                &format!(
                    "Restoring standard desktop window geometry: {}x{} at {},{}",
                    DEFAULT_DESKTOP_WIDTH, DEFAULT_DESKTOP_HEIGHT, target_x, target_y
                ),
            );

            unsafe {
                let _ = ReleaseCapture();
                if let Ok(arrow) = LoadCursorW(None, IDC_ARROW) {
                    let _ = SetCursor(arrow);
                }
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_NOTOPMOST,
                    target_x,
                    target_y,
                    DEFAULT_DESKTOP_WIDTH,
                    DEFAULT_DESKTOP_HEIGHT,
                    SWP_SHOWWINDOW | SWP_FRAMECHANGED,
                );
                let _ = SetForegroundWindow(hwnd);
            }
            return true;
        }
    }

    false
}

/// Returns true if at least one virtual display adapter (Sunshine/Apollo/IddSampleDriver) is currently connected.
pub fn is_virtual_display_present() -> bool {
    let monitors = enumerate_monitors();
    monitors.iter().any(|m| m.is_virtual)
}

/// Checks whether a previously identified streaming display is still present in the active monitor list.
#[allow(dead_code)]
pub fn is_streaming_display_active(target_name: Option<&str>) -> bool {
    // 1. If explicit Sunshine/Moonlight client resolution env vars are set, streaming is actively running
    if std::env::var("SUNSHINE_CLIENT_WIDTH").is_ok() || std::env::var("SUNSHINE_CLIENT_HEIGHT").is_ok() {
        return true;
    }

    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        return false;
    }
    if let Some(target) = target_name {
        if !target.is_empty() {
            return monitors.iter().any(|m| m.device_name == target || m.description == target);
        }
    }
    // If no specific device name and no active streaming session, do not treat idle virtual drivers as active streams
    false
}

/// Checks whether the DLSS Studio window coordinates fall completely outside of any active monitor.
/// If stranded (e.g. following monitor unplug or resolution shift), snaps it back to the primary display.
pub fn ensure_window_not_stranded() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

        if let Some(hwnd) = get_app_window_hwnd() {
            let mut win_rect = RECT::default();
            unsafe {
                if GetWindowRect(hwnd, &mut win_rect).is_ok() {
                    let monitors = enumerate_monitors();
                    let overlaps_any = monitors.iter().any(|m| {
                        let mon_right = m.left + m.width;
                        let mon_bottom = m.top + m.height;
                        win_rect.left < mon_right
                            && win_rect.right > m.left
                            && win_rect.top < mon_bottom
                            && win_rect.bottom > m.top
                    });

                    if !overlaps_any {
                        crate::core::logger::warn(
                            "display",
                            &format!(
                                "Window at ({},{} - {},{}) is stranded outside all monitors. Restoring geometry.",
                                win_rect.left, win_rect.top, win_rect.right, win_rect.bottom
                            ),
                        );
                        return restore_desktop_window_geometry();
                    }
                }
            }
        }
    }

    false
}




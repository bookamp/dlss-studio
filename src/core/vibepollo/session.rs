//! Vibepollo / Sunshine / Apollo Streaming Session Lifecycle & Process Bridge.
//!
//! Provides Win32 named Event synchronization so secondary processes spawned by Sunshine/Apollo
//! can hold the stream process lifecycle until Big Picture mode exits, signaling stream teardown.

#[cfg(windows)]
use windows::Win32::Foundation::CloseHandle;
#[cfg(windows)]
use windows::Win32::System::Threading::{
    CreateEventW, OpenEventW, ResetEvent, SetEvent, WaitForSingleObject,
    EVENT_MODIFY_STATE, SYNCHRONIZATION_SYNCHRONIZE,
};
#[cfg(windows)]
use windows::core::PCWSTR;

pub const STREAM_SESSION_EVENT_NAME: &str = "Local\\DLSS5_Studio_StreamSession_Exit_Event";

/// Evaluates whether the current instance is running inside an active Sunshine/Moonlight streaming session.
pub fn is_streaming_session() -> bool {
    // 1. Sunshine injected environment variables
    if std::env::var("SUNSHINE_CLIENT_WIDTH").is_ok()
        || std::env::var("SUNSHINE_CLIENT_HEIGHT").is_ok()
        || std::env::var("SUNSHINE_APP_ID").is_ok()
    {
        return true;
    }

    // 2. Active virtual monitor check
    crate::core::platform::display::is_virtual_display_present()
}

#[cfg(windows)]
pub fn get_main_app_process_handle() -> Option<windows::Win32::Foundation::HANDLE> {
    unsafe {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
        use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId};

        let class_name = windows::core::w!("DLSS5StudioTrayClass");
        let window_name = windows::core::w!("DLSS5StudioTrayWindow");
        if let Ok(hwnd) = FindWindowW(class_name, window_name) {
            if !hwnd.0.is_null() {
                let mut pid: u32 = 0;
                let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
                if pid > 0 && pid != std::process::id() {
                    if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
                        if !handle.is_invalid() {
                            return Some(handle);
                        }
                    }
                }
            }
        }
        None
    }
}

/// Waits until the primary instance signals that Big Picture has exited, or until timeout (e.g. INFINITE = 0xFFFFFFFF).
/// Used by secondary instances spawned as `cmd` by Sunshine to hold the streaming session alive.
pub fn wait_for_stream_session_exit(timeout_ms: u32) -> bool {
    wait_for_stream_session_exit_named(STREAM_SESSION_EVENT_NAME, timeout_ms)
}

pub fn wait_for_stream_session_exit_named(event_name: &str, timeout_ms: u32) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::WaitForMultipleObjects;

        let wide_name: Vec<u16> = event_name.encode_utf16().chain(std::iter::once(0)).collect();
        // Create manual-reset event initially nonsignaled
        let event_handle = match CreateEventW(None, true, false, PCWSTR::from_raw(wide_name.as_ptr())) {
            Ok(h) if !h.is_invalid() => h,
            _ => {
                crate::core::logger::warn("vibepollo", "Failed to create stream session exit event");
                return false;
            }
        };

        crate::core::logger::info(
            "vibepollo",
            &format!("Stream session holder attached to '{}'; awaiting Big Picture exit signal or process death...", event_name),
        );

        let main_proc_handle = get_main_app_process_handle();
        let wait_res = if let Some(proc_handle) = main_proc_handle {
            let handles = [event_handle, proc_handle];
            let res = WaitForMultipleObjects(&handles, false, timeout_ms);
            let _ = CloseHandle(proc_handle);
            res
        } else {
            WaitForSingleObject(event_handle, timeout_ms)
        };

        let _ = CloseHandle(event_handle);

        crate::core::logger::info(
            "vibepollo",
            &format!("Stream session holder released for '{}' (wait_res: {:?}); process exiting cleanly.", event_name, wait_res),
        );

        wait_res.0 == 0 || wait_res.0 == 1
    }

    #[cfg(not(windows))]
    {
        let _ = event_name;
        let _ = timeout_ms;
        true
    }
}

/// Signals the stream session exit event to unblock any waiting secondary session holder.
pub fn signal_stream_session_exit() {
    signal_stream_session_exit_named(STREAM_SESSION_EVENT_NAME);
}

pub fn signal_stream_session_exit_named(event_name: &str) {
    #[cfg(windows)]
    unsafe {
        let wide_name: Vec<u16> = event_name.encode_utf16().chain(std::iter::once(0)).collect();
        // Try to open existing event or create and set it
        if let Ok(handle) = OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR::from_raw(wide_name.as_ptr())) {
            if !handle.is_invalid() {
                let _ = SetEvent(handle);
                let _ = CloseHandle(handle);
                crate::core::logger::info("vibepollo", &format!("Signaled stream session exit event for '{}'.", event_name));
                return;
            }
        }

        // If not opened, create and set to ensure any imminent waiter unblocks
        if let Ok(handle) = CreateEventW(None, true, true, PCWSTR::from_raw(wide_name.as_ptr())) {
            if !handle.is_invalid() {
                let _ = SetEvent(handle);
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Resets the stream session exit event back to non-signaled state when starting Big Picture.
pub fn reset_stream_session_event() {
    reset_stream_session_event_named(STREAM_SESSION_EVENT_NAME);
}

pub fn reset_stream_session_event_named(event_name: &str) {
    #[cfg(windows)]
    unsafe {
        let wide_name: Vec<u16> = event_name.encode_utf16().chain(std::iter::once(0)).collect();
        if let Ok(handle) = CreateEventW(None, true, false, PCWSTR::from_raw(wide_name.as_ptr())) {
            if !handle.is_invalid() {
                let _ = ResetEvent(handle);
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Spawns a new independent, detached DLSS Studio process in standard desktop mode.
/// Used when exiting Big Picture mode from a Sunshine/Moonlight stream to ensure DLSS Studio
/// remains running on the host desktop after the streaming session process exits.
pub fn spawn_detached_desktop_instance() -> bool {
    let curr_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            crate::core::logger::warn("vibepollo", &format!("Failed to resolve current exe for detached spawn: {}", e));
            return false;
        }
    };

    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let exe_wide: Vec<u16> = curr_exe.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        let mut exec_info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            hwnd: windows::Win32::Foundation::HWND::default(),
            lpVerb: PCWSTR::null(),
            lpFile: PCWSTR::from_raw(exe_wide.as_ptr()),
            lpParameters: PCWSTR::null(),
            lpDirectory: PCWSTR::null(),
            nShow: SW_SHOWNORMAL.0 as i32,
            ..Default::default()
        };

        let res = unsafe { ShellExecuteExW(&mut exec_info) };
        if res.is_ok() {
            crate::core::logger::info("vibepollo", &format!("Spawned detached desktop instance: {}", curr_exe.display()));
            true
        } else {
            crate::core::logger::warn("vibepollo", &format!("ShellExecuteExW failed: {:?}", res));
            false
        }
    }

    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new(curr_exe).spawn();
        true
    }
}

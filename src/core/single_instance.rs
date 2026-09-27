#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, LPARAM, WPARAM};
#[cfg(windows)]
use windows::Win32::System::Threading::CreateMutexW;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, SetForegroundWindow, ShowWindow, SW_RESTORE};

pub struct SingleInstanceGuard {
    #[cfg(windows)]
    handle: HANDLE,
}

#[cfg(windows)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

/// Acquires a named Windows mutex. If the mutex is already held by another process, returns `None`.
pub fn acquire_named_mutex(mutex_name: PCWSTR) -> Option<SingleInstanceGuard> {
    #[cfg(windows)]
    unsafe {
        let handle = match CreateMutexW(None, true, mutex_name) {
            Ok(h) => h,
            Err(e) => {
                crate::core::logger::warn("single_instance", &format!("CreateMutexW failed: {:?}", e));
                return None;
            }
        };

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }

        Some(SingleInstanceGuard { handle })
    }

    #[cfg(not(windows))]
    None
}

/// Tries to acquire single-instance ownership for DLSS 5 Studio.
/// If an instance is already running, signals the running instance to restore/show its window and returns `None`.
/// If `is_big_picture` is true, signals the running instance to enter Big Picture mode directly.
/// If this is the primary instance, returns `Some(SingleInstanceGuard)`.
pub fn acquire_single_instance(is_big_picture: bool) -> Option<SingleInstanceGuard> {
    #[cfg(windows)]
    {
        let mutex_name = windows::core::w!("Local\\DLSS5_Studio_SingleInstance_Mutex");
        if let Some(guard) = acquire_named_mutex(mutex_name) {
            Some(guard)
        } else {
            signal_existing_instance(is_big_picture);
            None
        }
    }

    #[cfg(not(windows))]
    None
}

/// Signals the existing instance via its Win32 tray/message window to restore and focus its UI.
pub fn signal_existing_instance(is_big_picture: bool) {
    #[cfg(windows)]
    unsafe {
        let class_name = windows::core::w!("DLSS5StudioTrayClass");
        let window_name = windows::core::w!("DLSS5StudioTrayWindow");
        if let Ok(hwnd) = FindWindowW(class_name, window_name) {
            if !hwnd.0.is_null() {
                let msg = if is_big_picture {
                    crate::core::tray::WM_SHOW_BIG_PICTURE
                } else {
                    crate::core::tray::WM_SHOW_WINDOW
                };
                let wparam = if is_big_picture { WPARAM(1) } else { WPARAM(0) };
                let _ = PostMessageW(hwnd, msg, wparam, LPARAM(0));
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
                crate::core::logger::info(
                    "single_instance",
                    &format!(
                        "Signaled existing instance via Win32 tray window (big_picture: {})",
                        is_big_picture
                    ),
                );
            }
        }
    }
}


use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO,
    NIM_ADD, NIM_DELETE, NIM_MODIFY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    SetMenuDefaultItem, TrackPopupMenu, TranslateMessage, IDI_APPLICATION, MF_CHECKED,
    MF_SEPARATOR, MF_STRING, MF_UNCHECKED, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_APP, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP,
    WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

const WM_TRAY_CALLBACK: u32 = WM_APP + 101;
pub const WM_SHOW_WINDOW: u32 = WM_APP + 102;
pub const WM_SHOW_BIG_PICTURE: u32 = WM_APP + 103;
const TRAY_ICON_ID: u32 = 1001;

const CMD_OPEN: u32 = 2001;
const CMD_RUN_ON_STARTUP: u32 = 2002;
const CMD_EXIT: u32 = 2003;

static SHOW_WINDOW_REQUESTED: AtomicBool = AtomicBool::new(false);
static SHOW_BIG_PICTURE_REQUESTED: AtomicBool = AtomicBool::new(false);
static TRAY_HWND: Mutex<Option<isize>> = Mutex::new(None);

pub fn request_show_window() {
    crate::core::logger::info("tray", "Show window requested from system tray");
    SHOW_WINDOW_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn take_show_window_request() -> bool {
    SHOW_WINDOW_REQUESTED.swap(false, Ordering::SeqCst)
}

pub fn request_show_big_picture() {
    crate::core::logger::info("tray", "Show Big Picture requested via IPC / Moonlight");
    SHOW_BIG_PICTURE_REQUESTED.store(true, Ordering::SeqCst);
    SHOW_WINDOW_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn take_show_big_picture_request() -> bool {
    SHOW_BIG_PICTURE_REQUESTED.swap(false, Ordering::SeqCst)
}

#[cfg(windows)]
pub fn trim_working_set() {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcess, SetProcessWorkingSetSize,
        PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::Foundation::CloseHandle;

    unsafe {
        let current_process = GetCurrentProcess();
        let _ = SetProcessWorkingSetSize(current_process, usize::MAX, usize::MAX);

        let my_pid = GetCurrentProcessId();
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32ParentProcessID == my_pid {
                        if let Ok(child_handle) =
                            OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, false, entry.th32ProcessID)
                        {
                            let _ = SetProcessWorkingSetSize(child_handle, usize::MAX, usize::MAX);
                            let _ = CloseHandle(child_handle);
                        }
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }
    crate::core::logger::info("tray", "Working set trimmed for main and child WebView2 processes");
}

#[cfg(not(windows))]
pub fn trim_working_set() {}

#[doc(hidden)]
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------- Registry: Run on Startup ----------------

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_NAME: &str = "DLSS5Studio";

pub fn is_startup_enabled() -> bool {
    unsafe {
        let subkey = to_wide(RUN_KEY);
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
            || hkey.is_invalid()
        {
            return false;
        }

        let val_name = to_wide(APP_NAME);
        let mut val_type = REG_VALUE_TYPE::default();
        let mut data_len = 0u32;
        let status = RegQueryValueExW(
            hkey,
            PCWSTR(val_name.as_ptr()),
            None,
            Some(&mut val_type),
            None,
            Some(&mut data_len),
        );
        let _ = RegCloseKey(hkey);
        status.is_ok()
    }
}

pub fn set_startup_enabled(enable: bool) -> Result<(), String> {
    unsafe {
        let subkey = to_wide(RUN_KEY);
        let mut hkey = HKEY::default();
        let open_res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_WRITE | KEY_READ,
            &mut hkey,
        );
        if open_res.is_err() || hkey.is_invalid() {
            return Err("Failed to open HKCU Run registry key".to_string());
        }

        let val_name = to_wide(APP_NAME);

        let res = if enable {
            let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let cmd = format!("\"{}\" --background", current_exe.display());
            let val_data = to_wide(&cmd);
            let byte_len = (val_data.len() * std::mem::size_of::<u16>()) as u32;

            let set_res = RegSetValueExW(
                hkey,
                PCWSTR(val_name.as_ptr()),
                0,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    val_data.as_ptr() as *const u8,
                    byte_len as usize,
                )),
            );
            if set_res.is_ok() {
                Ok(())
            } else {
                Err(format!("RegSetValueExW failed: {:?}", set_res))
            }
        } else {
            let del_res = RegDeleteValueW(hkey, PCWSTR(val_name.as_ptr()));
            if del_res.is_ok() {
                Ok(())
            } else {
                Err(format!("RegDeleteValueW failed: {:?}", del_res))
            }
        };

        let _ = RegCloseKey(hkey);
        crate::core::logger::info("tray", &format!("Configured Windows startup run key (enable={}): {:?}", enable, res));
        res
    }
}

// ---------------- Tray Management ----------------

#[doc(hidden)]
pub fn fill_u16_buf(dest: &mut [u16], text: &str) {
    let mut i = 0;
    for c in text.encode_utf16() {
        if i >= dest.len() - 1 {
            break;
        }
        dest[i] = c;
        i += 1;
    }
    dest[i] = 0;
}

pub fn show_background_notification() {
    let hwnd_raw = match TRAY_HWND.lock() {
        Ok(guard) => *guard,
        Err(_) => None,
    };
    let Some(hwnd_val) = hwnd_raw else { return };

    unsafe {
        let hwnd = HWND(hwnd_val as *mut _);
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_INFO;

        fill_u16_buf(&mut nid.szInfoTitle, "DLSS 5 Studio");
        fill_u16_buf(
            &mut nid.szInfo,
            "DLSS 5 Studio is now running in the background.",
        );

        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

pub fn start_system_tray() {
    std::thread::spawn(move || {
        unsafe {
            let class_name = w!("DLSS5StudioTrayClass");
            let icon = GetModuleHandleW(None)
                .ok()
                .and_then(|h| LoadIconW(h, PCWSTR(1 as *const u16)).ok())
                .unwrap_or_else(|| LoadIconW(None, IDI_APPLICATION).unwrap_or_default());

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(tray_wnd_proc),
                hInstance: Default::default(),
                lpszClassName: class_name,
                hIcon: icon,
                ..Default::default()
            };

            let _ = RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class_name,
                w!("DLSS5StudioTrayWindow"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap_or_default();

            if hwnd.0.is_null() {
                return;
            }

            if let Ok(mut guard) = TRAY_HWND.lock() {
                *guard = Some(hwnd.0 as isize);
            }

            // Register tray icon
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY_CALLBACK;
            nid.hIcon = icon;
            fill_u16_buf(&mut nid.szTip, "DLSS 5 Studio (Running in background)");

            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            // Message pump
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Clean up
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
    });
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SHOW_WINDOW => {
            if wparam.0 == 1 {
                request_show_big_picture();
            } else {
                request_show_window();
            }
            LRESULT(0)
        }
        WM_SHOW_BIG_PICTURE => {
            request_show_big_picture();
            LRESULT(0)
        }
        WM_TRAY_CALLBACK => {
            let event = (lparam.0 & 0xffff) as u32;
            if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                show_tray_context_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK || event == WM_LBUTTONUP {
                request_show_window();
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_tray_context_menu(hwnd: HWND) {
    let mut pt = POINT::default();
    if GetCursorPos(&mut pt).is_err() {
        return;
    }

    let hmenu = match CreatePopupMenu() {
        Ok(m) => m,
        Err(_) => return,
    };

    let _ = AppendMenuW(
        hmenu,
        MF_STRING,
        CMD_OPEN as usize,
        w!("Open DLSS 5 Studio"),
    );

    let startup_checked = if is_startup_enabled() {
        MF_CHECKED
    } else {
        MF_UNCHECKED
    };
    let _ = AppendMenuW(
        hmenu,
        MF_STRING | startup_checked,
        CMD_RUN_ON_STARTUP as usize,
        w!("Run on startup"),
    );

    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, None);

    let _ = AppendMenuW(
        hmenu,
        MF_STRING,
        CMD_EXIT as usize,
        w!("Exit DLSS 5 Studio"),
    );

    let _ = SetMenuDefaultItem(hmenu, CMD_OPEN, 0);

    // Required by Windows so clicking outside menu dismisses it
    let _ = SetForegroundWindow(hwnd);

    let chosen = TrackPopupMenu(
        hmenu,
        TPM_RIGHTBUTTON | TPM_NONOTIFY | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );

    let _ = DestroyMenu(hmenu);

    let cmd = chosen.0 as u32;
    if cmd == CMD_OPEN {
        request_show_window();
    } else if cmd == CMD_RUN_ON_STARTUP {
        let currently_on = is_startup_enabled();
        let _ = set_startup_enabled(!currently_on);
    } else if cmd == CMD_EXIT {
        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        std::process::exit(0);
    }
}


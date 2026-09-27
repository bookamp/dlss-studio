// Lightweight thread-safe logger writing to dlss-mip-fix.log next to the add-on DLL.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init_logger(addon_module: isize) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;

    let mut buf = [0u16; 512];
    let len = unsafe { GetModuleFileNameW(addon_module as _, buf.as_mut_ptr(), buf.len() as u32) };
    if len > 0 {
        let full_path = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
        if let Some(parent) = full_path.parent() {
            let log_file = parent.join("dlss-mip-fix.log");
            if let Ok(mut lock) = LOG_PATH.lock() {
                *lock = Some(log_file);
            }
        }
    }
}

pub fn log(msg: &str) {
    let path = match LOG_PATH.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    };

    let path = match path {
        Some(p) => p,
        None => return,
    };

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[dlss-mip-fix] {}", msg);
    }
}

#[macro_export]
macro_rules! addon_log {
    ($($arg:tt)*) => {
        $crate::log::log(&format!($($arg)*))
    };
}

//! Pure Rust Game Process Lifecycle Supervisor for DLSS Studio Big Picture Mode.
//!
//! Spawns the game executable directly (bypassing desktop launcher popups),
//! coordinates with Vibepollo / Moonlight streaming by minimizing DLSS Studio during play,
//! monitors the game process tree, and automatically restores Big Picture fullscreen upon exit.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crate::core::install_guards::get_running_processes;

#[derive(Debug, Clone)]
pub struct GameLaunchSession {
    pub game_title: String,
    pub exe_path: PathBuf,
    pub is_running: Arc<AtomicBool>,
}

/// Spawns the game executable directly in its home directory and runs a background
/// supervisor loop that waits for completion, then invokes `on_exit`.
pub fn launch_and_supervise<F>(
    title: &str,
    target_exe: &Path,
    on_launch: impl FnOnce() + Send + 'static,
    on_exit: F,
) -> Result<GameLaunchSession, String>
where
    F: FnOnce() + Send + 'static,
{
    if !target_exe.is_file() {
        return Err(format!("Target executable does not exist: {}", target_exe.display()));
    }

    let parent_dir = target_exe.parent().unwrap_or_else(|| Path::new("."));
    let mut child = std::process::Command::new(target_exe)
        .current_dir(parent_dir)
        .spawn()
        .map_err(|e| format!("Failed to spawn game {}: {}", target_exe.display(), e))?;

    let child_pid = child.id();
    crate::big_picture::logger::info(
        "supervisor",
        &format!("Spawning game '{}' [PID {}] -> {}", title, child_pid, target_exe.display()),
    );

    let exe_name = target_exe
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let is_running = Arc::new(AtomicBool::new(true));
    let session = GameLaunchSession {
        game_title: title.to_string(),
        exe_path: target_exe.to_path_buf(),
        is_running: is_running.clone(),
    };

    // Notify caller that game is launched (e.g. minimize window for Vibepollo capture)
    on_launch();

    let title_clone = title.to_string();
    tokio::spawn(async move {
        // Initial grace period to allow the game engine window / swapchain to initialize
        tokio::time::sleep(Duration::from_millis(1500)).await;

        let mut daughter_logged = false;
        loop {
            // Check if the directly spawned child is still alive
            let child_alive = match child.try_wait() {
                Ok(Some(_status)) => false,
                Ok(None) => true,
                Err(_) => false,
            };

            if child_alive {
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }

            // If the initial child exited, check the process snapshot to see if
            // a daughter process matching the game binary name is still active
            if !exe_name.is_empty() {
                let target_lower = exe_name.to_lowercase();
                let running = get_running_processes();
                let matching = running.iter().any(|p| p.name.to_lowercase() == target_lower);

                if matching {
                    if !daughter_logged {
                        crate::big_picture::logger::info(
                            "supervisor",
                            &format!("Initial launcher PID {} exited; supervising active game process '{}'", child_pid, exe_name),
                        );
                        daughter_logged = true;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    continue;
                }
            }

            // Game is fully closed
            break;
        }

        is_running.store(false, Ordering::SeqCst);
        crate::big_picture::logger::info(
            "supervisor",
            &format!("Game session ended for '{}'. Cleaning up and notifying Big Picture...", title_clone),
        );

        // Terminate any orphaned dlss5-feed-host64.exe helper from this session
        cleanup_feeder_helper();

        // Invoke exit callback (e.g. un-minimize and restore fullscreen)
        on_exit();
    });

    Ok(session)
}

/// Cleans up any orphaned `dlss5-feed-host64.exe` processes left behind when a game terminates.
#[cfg(windows)]
pub fn cleanup_feeder_helper() {
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::Foundation::CloseHandle;

    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                    if name == "dlss5-feed-host64.exe" {
                        if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID) {
                            let _ = TerminateProcess(handle, 0);
                            let _ = CloseHandle(handle);
                            crate::big_picture::logger::info(
                                "supervisor",
                                &format!("Terminated orphaned feeder helper PID {}", entry.th32ProcessID),
                            );
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
}

#[cfg(not(windows))]
pub fn cleanup_feeder_helper() {}

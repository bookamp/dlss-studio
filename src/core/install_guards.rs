use std::path::Path;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::Foundation::CloseHandle;

#[derive(Debug, Clone)]
pub struct ProcessMatch {
    pub name: String,
}

pub fn get_running_processes() -> Vec<ProcessMatch> {
    let mut matches = Vec::new();
    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                    matches.push(ProcessMatch {
                        name,
                    });

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }
    matches
}

pub fn assert_game_closed(_game_dir: &Path, exe_path: Option<&Path>) -> Result<(), String> {
    let processes = get_running_processes();
    let target_name = exe_path
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase());

    for proc in processes {
        let p_name = proc.name.to_lowercase();
        if let Some(ref target) = target_name {
            if p_name == *target || p_name == "dlss5-feed-host64.exe" {
                return Err(format!("Close the game and helper first: {}", proc.name));
            }
        }
    }
    Ok(())
}

const ANTI_CHEAT_NAMES: &[&str] = &[
    "easyanticheat",
    "eac_server",
    "battleye",
    "beservice",
    "vgk",
    "vanguard",
    "start_protected_game",
    "denuvo",
    "equ8",
    "ricochet",
    "mhyprot",
    "ace-base",
];

pub fn has_anti_cheat(game_dir: &Path) -> bool {
    let mut dirs_to_check = vec![game_dir.to_path_buf()];
    let mut examined = 0;

    while let Some(dir) = dirs_to_check.pop() {
        if examined > 500 {
            break;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                examined += 1;
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_lowercase();

                for ac in ANTI_CHEAT_NAMES {
                    if file_name.contains(ac) {
                        return true;
                    }
                }

                if path.is_dir() && examined < 500 {
                    let name_str = file_name.as_str();
                    if !["paks", "movies", "saves", "logs", ".git"].contains(&name_str) {
                        dirs_to_check.push(path);
                    }
                }
            }
        }
    }
    false
}


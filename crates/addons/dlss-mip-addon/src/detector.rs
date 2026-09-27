// Smart Multi-Module NGX Scanner & Detour Resolution.
//
// Solves the Streamline 1.x module collision affecting A Plague Tale: Requiem:
// In Streamline 1.x, `sl.common.dll` exports `NVSDK_NGX_D3D12_EvaluateFeature` as an unhooked proxy
// and appears in the process module list before `_nvngx.dll`.
// Naive scanners that pick the first module exporting the symbol will pick `sl.common.dll`,
// find it un-detoured, and fail to hook.
// This module scans all candidates and locks onto the one RenoDX actually detoured.

use crate::hook::is_detoured;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCandidate {
    pub name: String,
    pub base_address: usize,
    pub eval_entry_bytes: Vec<u8>,
}

/// Returns true if the module name represents the core NVIDIA NGX runtime driver module
/// (`_nvngx.dll` or `nvngx.dll`).
///
/// DLSS feature plugins (e.g., `nvngx_dlss.dll`, `nvngx_dlssg.dll`, `nvngx_dlssnr.dll`)
/// and proxy shims (e.g., `sl.common.dll`) are NOT the NGX runtime loader and must
/// never be hooked by this companion add-on.
pub fn is_ngx_runtime_module(name: &str) -> bool {
    let lower = name.to_lowercase();
    let file_name = lower.rsplit(|c| c == '/' || c == '\\').next().unwrap_or(&lower);
    file_name == "_nvngx.dll" || file_name == "nvngx.dll"
}

/// Selects the module candidate whose `NVSDK_NGX_D3D12_EvaluateFeature` entry point
/// has actually been detoured by an upstream add-on (e.g., RenoDX).
///
/// Strictly isolates to genuine NGX runtime driver modules (`_nvngx.dll` / `nvngx.dll`).
/// If multiple runtime modules are detoured, prioritizes `_nvngx.dll`.
pub fn select_detoured_module<'a>(candidates: &'a [ModuleCandidate]) -> Option<&'a ModuleCandidate> {
    let mut detoured: Vec<&'a ModuleCandidate> = candidates
        .iter()
        .filter(|c| is_ngx_runtime_module(&c.name) && is_detoured(&c.eval_entry_bytes))
        .collect();

    if detoured.is_empty() {
        return None;
    }

    // If _nvngx.dll is among the detoured candidates, prefer it.
    if let Some(&nvngx) = detoured
        .iter()
        .find(|c| c.name.to_lowercase().contains("_nvngx.dll"))
    {
        return Some(nvngx);
    }

    // Otherwise return the first detoured candidate
    Some(detoured.remove(0))
}

/// Enumerates all modules in the current process, finds all exporting
/// `NVSDK_NGX_D3D12_EvaluateFeature`, and returns the HMODULE and function pointer
/// of the module that has actually been detoured.
pub unsafe fn find_detoured_ngx_module() -> Option<(isize, *mut u8)> {
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleFileNameW, GetProcAddress};
    use windows_sys::Win32::System::ProcessStatus::K32EnumProcessModules;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut mods = [0 as HMODULE; 1024];
    let mut needed = 0u32;

    if K32EnumProcessModules(
        GetCurrentProcess(),
        mods.as_mut_ptr(),
        (mods.len() * std::mem::size_of::<HMODULE>()) as u32,
        &mut needed,
    ) == 0
    {
        return None;
    }

    let count = (needed as usize) / std::mem::size_of::<HMODULE>();
    let eval_sym = b"NVSDK_NGX_D3D12_EvaluateFeature\0";

    let mut candidates: Vec<(isize, *mut u8, ModuleCandidate)> = Vec::new();

    for &hmod in &mods[..count] {
        if hmod == 0 {
            continue;
        }

        let p_eval = GetProcAddress(hmod, eval_sym.as_ptr());
        if p_eval.is_none() {
            continue;
        }

        let eval_ptr = p_eval.unwrap() as *mut u8;

        // Read first 14 bytes safely
        let mut entry_bytes = [0u8; 14];
        std::ptr::copy_nonoverlapping(eval_ptr, entry_bytes.as_mut_ptr(), 14);

        let mut path_buf = [0u16; 512];
        let len = GetModuleFileNameW(hmod, path_buf.as_mut_ptr(), path_buf.len() as u32);
        let mod_name = if len > 0 {
            String::from_utf16_lossy(&path_buf[..len as usize])
        } else {
            format!("mod_{:#x}", hmod as usize)
        };

        let is_runtime = is_ngx_runtime_module(&mod_name);
        let detoured = is_detoured(&entry_bytes);
        crate::log::log(&format!(
            "[detector] module candidate: {} (eval={:p}, runtime={}, detoured={})",
            mod_name, eval_ptr, is_runtime, detoured
        ));

        candidates.push((
            hmod as isize,
            eval_ptr,
            ModuleCandidate {
                name: mod_name,
                base_address: hmod as usize,
                eval_entry_bytes: entry_bytes.to_vec(),
            },
        ));
    }

    // Extract module candidates for decision
    let candidate_structs: Vec<ModuleCandidate> =
        candidates.iter().map(|(_, _, c)| c.clone()).collect();

    let selected = select_detoured_module(&candidate_structs)?;

    crate::log::log(&format!(
        "[detector] selected authoritative NGX module: {} at base {:#x}",
        selected.name, selected.base_address
    ));

    // Find the corresponding HMODULE and eval_ptr
    for (hmod, ptr, c) in candidates {
        if c.base_address == selected.base_address {
            return Some((hmod, ptr));
        }
    }

    None
}


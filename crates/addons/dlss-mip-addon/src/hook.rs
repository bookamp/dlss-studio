// Inline 14-byte x64 detour hooking and instruction inspection.

use std::sync::atomic::{AtomicBool, Ordering};

/// Checks whether the machine code at the function entry begins with a jump or detour sequence.
///
/// Recognized patterns:
/// - `FF 25 00 00 00 00 [64-bit target]` : x64 RIP-relative absolute jump (used by MinHook, RenoDX, and ReShade)
/// - `E9 [32-bit relative offset]`       : 32-bit relative near jump
/// - `EB [8-bit relative offset]`        : 8-bit relative short jump
/// - `48 B8 [64-bit imm] FF E0`          : `mov rax, <addr>; jmp rax`
pub fn is_detoured(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    // 0xFF 0x25 0x00 0x00 0x00 0x00: jmp qword ptr [rip + 0]
    if bytes.len() >= 6
        && bytes[0] == 0xFF
        && bytes[1] == 0x25
        && bytes[2] == 0x00
        && bytes[3] == 0x00
        && bytes[4] == 0x00
        && bytes[5] == 0x00
    {
        return true;
    }

    // 0xE9: jmp rel32
    if bytes[0] == 0xE9 && bytes.len() >= 5 {
        return true;
    }

    // 0xEB: jmp rel8
    if bytes[0] == 0xEB && bytes.len() >= 2 {
        return true;
    }

    // 0x48 0xB8: mov rax, imm64 (10 bytes)
    if bytes[0] == 0x48 && bytes.len() >= 2 && bytes[1] == 0xB8 {
        return true;
    }

    false
}

/// 14-byte x64 absolute jump hook structure
pub struct Hook {
    pub target: *mut u8,
    pub saved: [u8; 14],
    pub patch: [u8; 14],
    pub active: AtomicBool,
}

unsafe impl Send for Hook {}
unsafe impl Sync for Hook {}

impl Hook {
    pub const fn new() -> Self {
        Self {
            target: std::ptr::null_mut(),
            saved: [0u8; 14],
            patch: [0u8; 14],
            active: AtomicBool::new(false),
        }
    }

    /// Prepares patch bytes for `jmp qword ptr [rip + 0]` targeting `detour`.
    pub fn build_patch(detour: usize) -> [u8; 14] {
        let mut patch = [0u8; 14];
        patch[0] = 0xFF; // jmp qword ptr [rip + 0]
        patch[1] = 0x25;
        patch[2] = 0x00;
        patch[3] = 0x00;
        patch[4] = 0x00;
        patch[5] = 0x00;
        let detour_bytes = detour.to_ne_bytes();
        patch[6..14].copy_from_slice(&detour_bytes);
        patch
    }

    /// Installs the hook on `target` pointing to `detour`.
    ///
    /// # Safety
    /// Modifies executable code page permissions via VirtualProtect.
    pub unsafe fn install(&mut self, target: *mut u8, detour: usize) -> bool {
        if target.is_null() {
            return false;
        }

        self.target = target;
        std::ptr::copy_nonoverlapping(target, self.saved.as_mut_ptr(), 14);
        self.patch = Self::build_patch(detour);

        if !write_memory(target, &self.patch) {
            return false;
        }

        self.active.store(true, Ordering::SeqCst);
        true
    }

    /// Temporarily uninstalls the hook by restoring saved bytes.
    ///
    /// # Safety
    /// Must only be called if hook was successfully installed.
    pub unsafe fn remove(&self) {
        if self.active.load(Ordering::SeqCst) && !self.target.is_null() {
            let _ = write_memory(self.target, &self.saved);
        }
    }

    /// Re-applies the patch after the forwarded call completes.
    ///
    /// # Safety
    /// Must only be called if hook was successfully installed.
    pub unsafe fn restore(&self) {
        if self.active.load(Ordering::SeqCst) && !self.target.is_null() {
            let _ = write_memory(self.target, &self.patch);
        }
    }
}

/// Changes page protection, copies `src` into `dst`, restores protection, and flushes CPU cache.
unsafe fn write_memory(dst: *mut u8, src: &[u8]) -> bool {
    use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut old_protect = 0u32;
    if VirtualProtect(
        dst as _,
        src.len(),
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return false;
    }

    std::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());

    let mut dummy = 0u32;
    VirtualProtect(dst as _, src.len(), old_protect, &mut dummy);
    FlushInstructionCache(GetCurrentProcess(), dst as _, src.len());

    true
}


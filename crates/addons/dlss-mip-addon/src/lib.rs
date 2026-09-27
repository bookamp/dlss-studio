// Pure Rust D3D12 Mip-Chain Companion Add-on for DLSS Studio.
//
// Automatically bridges multi-mip D3D12 render targets into single-mip textures
// required by RenoDX DLSS 5 Neural Reconstruction, with robust multi-module detection
// for both A Plague Tale: Requiem and Resonance.

pub mod detector;
pub mod hook;
pub mod log;
pub mod substitute;

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::{BOOL, HMODULE};
use windows_sys::Win32::System::SystemServices::*;

#[no_mangle]
pub static NAME: &[u8] = b"DLSS Studio D3D12 Mip Companion 1.0.0\0";

#[no_mangle]
pub static DESCRIPTION: &[u8] = b"Bridges multi-mip DLSS output textures into single-mip targets for DLSS 5 Neural Reconstruction in D3D12 titles (e.g. Requiem and Resonance).\0";

static HOOK: Mutex<hook::Hook> = Mutex::new(hook::Hook::new());
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static EVAL_COUNT: AtomicI32 = AtomicI32::new(0);
static SUB_COUNT: AtomicI32 = AtomicI32::new(0);


struct FeatureEntry {
    handle: usize,
    id: i32,
}

static FEATURES: Mutex<Vec<FeatureEntry>> = Mutex::new(Vec::new());

#[doc(hidden)]
pub fn remember_feature(handle: *mut std::ffi::c_void, id: i32) {
    if handle.is_null() {
        return;
    }
    let mut f = FEATURES.lock().unwrap();
    if f.len() < 64 {
        f.push(FeatureEntry {
            handle: handle as usize,
            id,
        });
    }
}

fn feature_id_of(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        return -1;
    }
    let f = FEATURES.lock().unwrap();
    for entry in f.iter() {
        if entry.handle == handle as usize {
            return entry.id;
        }
    }
    -1
}

#[doc(hidden)]
pub fn should_substitute(handle: *mut std::ffi::c_void) -> bool {
    let id = feature_id_of(handle);
    if id < 0 {
        return true;
    }
    id == 1 || id == 11
}

unsafe extern "system" fn detour_evaluate(
    cmd_list: *mut std::ffi::c_void,
    handle: *mut std::ffi::c_void,
    params: *mut std::ffi::c_void,
) -> i32 {
    let eval_idx = EVAL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let allowed = should_substitute(handle);

    let sub_guard = substitute::EvaluateSubstitutionGuard::prepare(cmd_list, params, allowed);

    if let Some(ref g) = sub_guard {
        if g.did_out || g.did_dep {
            SUB_COUNT.fetch_add(1, Ordering::Relaxed);
            if eval_idx <= 5 || eval_idx % 3600 == 0 {
                log::log(&format!(
                    "[dlss-mip-fix] Substituted single-mip committed textures for eval #{}: Output={} Depth={}",
                    eval_idx, g.did_out, g.did_dep
                ));
            }
        }
    } else if eval_idx <= 5 {
        log::log(&format!(
            "[dlss-mip-fix] Eval #{}: params={:p}, sub_guard=None (not mipped or not allowed, handle={:p})",
            eval_idx, params, handle
        ));
    }

    if eval_idx % 3600 == 0 {
        log::log(&format!(
            "[dlss-mip-fix stats] Total evaluations: {}, substitutions: {}",
            eval_idx,
            SUB_COUNT.load(Ordering::Relaxed)
        ));
    }

    // Call through to original function by unhooking, invoking, and restoring
    let orig_fn: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
    ) -> i32;

    let res = {
        let guard = HOOK.lock().unwrap();
        guard.remove();
        orig_fn = std::mem::transmute(guard.target);
        let r = orig_fn(cmd_list, handle, params);
        guard.restore();
        r
    };

    if let Some(g) = sub_guard {
        g.complete();
    }

    res
}

static HOOK_CREATE: Mutex<hook::Hook> = Mutex::new(hook::Hook::new());
static HOOK_CREATE_INSTALLED: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn detour_create(
    cmd_list: *mut std::ffi::c_void,
    feature_id: i32,
    params: *mut std::ffi::c_void,
    out_handle: *mut *mut std::ffi::c_void,
) -> i32 {
    // For DLSS-NR (feature 18, NeuralRendering), ensure PerfQualityValue is 5 (DLAA)
    // when input resolution equals output resolution (1:1 scale), preventing 0xBAD00005.
    if feature_id == 18 && !params.is_null() {
        let fixed = substitute::fix_nr_quality_preset(params);
        if fixed {
            log::log("[dlss-mip-fix] Corrected PerfQualityValue to 5 (DLAA) for 1:1 NeuralRendering creation.");
        }
    }

    let orig_fn: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *mut std::ffi::c_void,
        *mut *mut std::ffi::c_void,
    ) -> i32;

    let res = {
        let guard = HOOK_CREATE.lock().unwrap();
        guard.remove();
        orig_fn = std::mem::transmute(guard.target);
        let r = orig_fn(cmd_list, feature_id, params, out_handle);
        guard.restore();
        r
    };

    if res == 1 && !out_handle.is_null() && !(*out_handle).is_null() {
        remember_feature(*out_handle, feature_id);
    }

    if feature_id == 18 {
        log::log(&format!(
            "[dlss-mip-fix] CreateFeature id=18 (NeuralRendering) result: {:#x}",
            res
        ));
    }

    res
}

pub unsafe fn try_install() -> bool {
    if HOOK_INSTALLED.load(Ordering::SeqCst) {
        return true;
    }

    if let Some((hmod, eval_ptr)) = detector::find_detoured_ngx_module() {
        // 1. Hook EvaluateFeature
        let mut guard = HOOK.lock().unwrap();
        if guard.install(eval_ptr, detour_evaluate as *const () as usize) {
            HOOK_INSTALLED.store(true, Ordering::SeqCst);
            log::log(&format!(
                "Successfully hooked NVSDK_NGX_D3D12_EvaluateFeature at {:p} (module {:#x})",
                eval_ptr, hmod
            ));
        }

        // 2. Hook CreateFeature if available
        use windows_sys::Win32::System::LibraryLoader::GetProcAddress;
        let create_ptr = GetProcAddress(hmod, b"NVSDK_NGX_D3D12_CreateFeature\0".as_ptr());
        if let Some(create_fn) = create_ptr {
            let mut create_guard = HOOK_CREATE.lock().unwrap();
            let target_addr = create_fn as *const () as *mut u8;
            if create_guard.install(target_addr, detour_create as *const () as usize) {
                HOOK_CREATE_INSTALLED.store(true, Ordering::SeqCst);
                log::log(&format!(
                    "Successfully hooked NVSDK_NGX_D3D12_CreateFeature at {:p}",
                    target_addr
                ));
            }
        }

        return true;
    }

    false
}

use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
};

static mut SELF_HMODULE: isize = 0;
static SHUTDOWN_SIGNAL: AtomicBool = AtomicBool::new(false);

pub unsafe fn pin_module(addr: *const ()) {
    let mut hmod = 0 as HMODULE;
    let res = GetModuleHandleExW(
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
        addr as *const u16,
        &mut hmod,
    );
    if res != 0 {
        log::log("[dlss-mip-fix] Module pinned in memory to survive temporary device rebuilds.");
    }
}

type PfnLdrRegisterDllNotification = unsafe extern "system" fn(
    flags: u32,
    callback: unsafe extern "system" fn(reason: u32, data: *const std::ffi::c_void, context: *mut std::ffi::c_void),
    context: *mut std::ffi::c_void,
    cookie: *mut *mut std::ffi::c_void,
) -> i32;

type PfnLdrUnregisterDllNotification = unsafe extern "system" fn(
    cookie: *mut std::ffi::c_void,
) -> i32;

static mut LDR_COOKIE: *mut std::ffi::c_void = std::ptr::null_mut();
static mut LDR_UNREGISTER: Option<PfnLdrUnregisterDllNotification> = None;

unsafe extern "system" fn on_ldr_dll_event(
    reason: u32,
    _data: *const std::ffi::c_void,
    _context: *mut std::ffi::c_void,
) {
    if SHUTDOWN_SIGNAL.load(Ordering::SeqCst) {
        return;
    }
    // 1 is LDR_DLL_NOTIFICATION_REASON_LOADED
    if reason == 1 {
        try_install();
    }
}

pub unsafe fn start_watching_for_ngx() {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

    let ntdll = GetModuleHandleA(b"ntdll.dll\0".as_ptr());
    if ntdll != 0 {
        if let Some(p_reg) = GetProcAddress(ntdll, b"LdrRegisterDllNotification\0".as_ptr()) {
            let reg_fn: PfnLdrRegisterDllNotification = std::mem::transmute(p_reg);
            let mut cookie: *mut std::ffi::c_void = std::ptr::null_mut();
            if reg_fn(0, on_ldr_dll_event, std::ptr::null_mut(), &mut cookie) == 0 {
                LDR_COOKIE = cookie;
                if let Some(p_unreg) = GetProcAddress(ntdll, b"LdrUnregisterDllNotification\0".as_ptr()) {
                    LDR_UNREGISTER = Some(std::mem::transmute(p_unreg));
                }
                log::log("Registered Windows loader DLL notification listener.");
            }
        }
    }

    // Try immediately in case module is already loaded
    try_install();

    // Also spawn a background retry loop to guarantee catching late initializers
    std::thread::spawn(|| {
        for _ in 0..150 {
            if SHUTDOWN_SIGNAL.load(Ordering::SeqCst) || HOOK_INSTALLED.load(Ordering::SeqCst) {
                break;
            }
            unsafe {
                if try_install() {
                    break;
                }
            }
            for _ in 0..5 {
                if SHUTDOWN_SIGNAL.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    });
}

#[no_mangle]
pub unsafe extern "system" fn ReShadeAddonInit(
    addon_module: HMODULE,
    _reshade_api: *const std::ffi::c_void,
) -> bool {
    SELF_HMODULE = addon_module as isize;
    pin_module(ReShadeAddonInit as *const ());
    log::init_logger(SELF_HMODULE);
    log::log("ReShadeAddonInit invoked for DLSS Studio D3D12 Mip Companion");
    start_watching_for_ngx();
    true
}

#[no_mangle]
pub unsafe extern "system" fn ReShadeAddonUnload(_addon_module: HMODULE) {
    log::log("ReShadeAddonUnload invoked for DLSS Studio D3D12 Mip Companion");
    SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst);
    let guard = HOOK.lock().unwrap();
    guard.remove();
    let create_guard = HOOK_CREATE.lock().unwrap();
    create_guard.remove();
}

type PfnReShadeRegister = unsafe extern "system" fn(HMODULE, u32) -> bool;
type PfnReShadeUnregister = unsafe extern "system" fn(HMODULE);

static mut RESHADE_UNREGISTER: Option<PfnReShadeUnregister> = None;

unsafe fn register_reshade(self_mod: HMODULE) {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows_sys::Win32::System::ProcessStatus::K32EnumProcessModules;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let reg_sym = b"ReShadeRegisterAddon\0";
    let unreg_sym = b"ReShadeUnregisterAddon\0";

    // 1. First probe common ReShade hook modules directly
    for &mod_name in &[
        b"dxgi.dll\0".as_ptr(),
        b"ReShade64.dll\0".as_ptr(),
        b"d3d12.dll\0".as_ptr(),
        b"d3d11.dll\0".as_ptr(),
    ] {
        let hmod = GetModuleHandleA(mod_name);
        if hmod != 0 {
            if let Some(p_reg) = GetProcAddress(hmod, reg_sym.as_ptr()) {
                let reg_fn: PfnReShadeRegister = std::mem::transmute(p_reg);
                for ver in (5..=18).rev() {
                    if reg_fn(self_mod, ver) {
                        if let Some(p_unreg) = GetProcAddress(hmod, unreg_sym.as_ptr()) {
                            RESHADE_UNREGISTER = Some(std::mem::transmute(p_unreg));
                        }
                        log::log(&format!("Registered as ReShade add-on (API v{})", ver));
                        return;
                    }
                }
            }
        }
    }

    // 2. Fall back to process module enumeration
    let mut mods = [0 as HMODULE; 1024];
    let mut needed = 0u32;
    if K32EnumProcessModules(
        GetCurrentProcess(),
        mods.as_mut_ptr(),
        (mods.len() * std::mem::size_of::<HMODULE>()) as u32,
        &mut needed,
    ) != 0
    {
        let count = (needed as usize) / std::mem::size_of::<HMODULE>();
        for &hmod in &mods[..count] {
            if hmod == 0 {
                continue;
            }
            if let Some(p_reg) = GetProcAddress(hmod, reg_sym.as_ptr()) {
                let reg_fn: PfnReShadeRegister = std::mem::transmute(p_reg);
                for ver in (5..=18).rev() {
                    if reg_fn(self_mod, ver) {
                        if let Some(p_unreg) = GetProcAddress(hmod, unreg_sym.as_ptr()) {
                            RESHADE_UNREGISTER = Some(std::mem::transmute(p_unreg));
                        }
                        log::log(&format!("Registered as ReShade add-on (API v{})", ver));
                        return;
                    }
                }
            }
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllMain(
    hinstance: HMODULE,
    fdw_reason: u32,
    _lpv_reserved: *mut std::ffi::c_void,
) -> BOOL {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            SELF_HMODULE = hinstance as isize;
            pin_module(hinstance as *const ());
            log::init_logger(SELF_HMODULE);
            log::log("DLSS Studio D3D12 Mip Companion attached.");

            register_reshade(hinstance);
            start_watching_for_ngx();
        }
        DLL_PROCESS_DETACH => {
            SHUTDOWN_SIGNAL.store(true, Ordering::SeqCst);
            if let Some(unreg) = LDR_UNREGISTER {
                if !LDR_COOKIE.is_null() {
                    unreg(LDR_COOKIE);
                    LDR_COOKIE = std::ptr::null_mut();
                }
            }

            log::log(&format!(
                "DLSS Studio D3D12 Mip Companion detaching. Total evaluations: {}, substitutions: {}",
                EVAL_COUNT.load(Ordering::Relaxed),
                SUB_COUNT.load(Ordering::Relaxed)
            ));

            let guard = HOOK.lock().unwrap();
            guard.remove();
            let create_guard = HOOK_CREATE.lock().unwrap();
            create_guard.remove();
        }
        _ => {}
    }

    1 // TRUE
}


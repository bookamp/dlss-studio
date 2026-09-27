use dlss_mip_addon::substitute::{
    fix_nr_quality_preset, ngx_param_get_d3d12_resource, ngx_param_set_d3d12_resource, RetiredQueue,
};
use std::sync::atomic::Ordering;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;

static MOCK_QUALITY_VALUE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static MOCK_RESOURCE_ADDR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

unsafe extern "system" fn mock_ngx_param_set_int(
    _this: *mut std::ffi::c_void,
    name: *const std::os::raw::c_char,
    val: i32,
) {
    if !name.is_null() {
        let c_str = std::ffi::CStr::from_ptr(name);
        if c_str.to_bytes() == b"PerfQualityValue" {
            MOCK_QUALITY_VALUE.store(val, Ordering::SeqCst);
        }
    }
}

unsafe extern "system" fn mock_ngx_param_set_res(
    _this: *mut std::ffi::c_void,
    name: *const std::os::raw::c_char,
    res: *mut std::ffi::c_void,
) {
    if !name.is_null() {
        let c_str = std::ffi::CStr::from_ptr(name);
        if c_str.to_bytes() == b"Output" || c_str.to_bytes() == b"Depth" {
            MOCK_RESOURCE_ADDR.store(res as usize, Ordering::SeqCst);
        }
    }
}

unsafe extern "system" fn mock_ngx_param_get_uint(
    _this: *const std::ffi::c_void,
    name: *const std::os::raw::c_char,
    out: *mut u32,
) -> i32 {
    if !name.is_null() && !out.is_null() {
        let c_str = std::ffi::CStr::from_ptr(name);
        if c_str.to_bytes() == b"Width" || c_str.to_bytes() == b"OutWidth" {
            *out = 3840;
            return 1;
        }
    }
    0
}

unsafe extern "system" fn mock_ngx_param_get_int(
    _this: *const std::ffi::c_void,
    name: *const std::os::raw::c_char,
    out: *mut i32,
) -> i32 {
    if !name.is_null() && !out.is_null() {
        let c_str = std::ffi::CStr::from_ptr(name);
        if c_str.to_bytes() == b"PerfQualityValue" {
            *out = MOCK_QUALITY_VALUE.load(Ordering::SeqCst);
            return 1;
        }
    }
    0
}

unsafe extern "system" fn mock_ngx_param_get_res(
    _this: *const std::ffi::c_void,
    name: *const std::os::raw::c_char,
    out: *mut *mut std::ffi::c_void,
) -> i32 {
    if !name.is_null() && !out.is_null() {
        let c_str = std::ffi::CStr::from_ptr(name);
        if c_str.to_bytes() == b"Output" || c_str.to_bytes() == b"Depth" {
            *out = MOCK_RESOURCE_ADDR.load(Ordering::SeqCst) as *mut std::ffi::c_void;
            return 1;
        }
    }
    0
}

#[test]
fn test_fix_nr_quality_preset_forces_dlaa_on_1to1_scale() {
    unsafe {
        MOCK_QUALITY_VALUE.store(0, Ordering::SeqCst);

        let mut vtable = Box::new([0usize; 20]);
        vtable[3] = mock_ngx_param_set_int as *const () as usize;
        vtable[11] = mock_ngx_param_get_int as *const () as usize;
        vtable[12] = mock_ngx_param_get_uint as *const () as usize;
        let mut lp_vtbl = vtable.as_mut_ptr();
        let param_ptr = &mut lp_vtbl as *mut *mut usize as *mut std::ffi::c_void;

        let modified = fix_nr_quality_preset(param_ptr);
        assert!(modified, "Must detect discrepancy and modify preset");
        assert_eq!(
            MOCK_QUALITY_VALUE.load(Ordering::SeqCst),
            5,
            "Must correct PerfQualityValue from 0 to 5 (DLAA) to prevent 0xBAD00005"
        );
    }
}

#[test]
fn test_ngx_param_get_and_set_resource_msvc_abi() {
    unsafe {
        let dummy_orig = 0x12345678 as *mut std::ffi::c_void;
        let dummy_sub = 0x87654321 as *mut std::ffi::c_void;
        MOCK_RESOURCE_ADDR.store(dummy_orig as usize, Ordering::SeqCst);

        let mut vtable = Box::new([0usize; 20]);
        vtable[1] = mock_ngx_param_set_res as *const () as usize;
        vtable[9] = mock_ngx_param_get_res as *const () as usize;
        let mut lp_vtbl = vtable.as_mut_ptr();
        let param_ptr = &mut lp_vtbl as *mut *mut usize as *mut std::ffi::c_void;

        // 1. Get original resource
        let res = ngx_param_get_d3d12_resource(param_ptr, "Output");
        assert!(res.is_some(), "Must retrieve resource from slot 9");
        let r = res.unwrap();
        assert_eq!(r.as_raw(), dummy_orig);
        std::mem::forget(r);

        // 2. Set substitute resource
        let sub_res = ID3D12Resource::from_raw(dummy_sub);
        ngx_param_set_d3d12_resource(param_ptr, "Output", Some(&sub_res));
        assert_eq!(
            MOCK_RESOURCE_ADDR.load(Ordering::SeqCst),
            dummy_sub as usize,
            "Slot 1 must have updated the pointer to substitute"
        );
        std::mem::forget(sub_res);

        // 3. Restore original resource
        let orig_res = ID3D12Resource::from_raw(dummy_orig);
        ngx_param_set_d3d12_resource(param_ptr, "Output", Some(&orig_res));
        assert_eq!(
            MOCK_RESOURCE_ADDR.load(Ordering::SeqCst),
            dummy_orig as usize,
            "Slot 1 must have restored original resource"
        );
        std::mem::forget(orig_res);
    }
}

#[test]
fn test_retired_queue_retires_and_drains_after_eight_frames() {
    let mut q = RetiredQueue::new();
    let dummy_raw1 = 0x1000 as *mut std::ffi::c_void;
    let dummy_raw2 = 0x2000 as *mut std::ffi::c_void;

    unsafe {
        let res1 = ID3D12Resource::from_raw(dummy_raw1);
        let res2 = ID3D12Resource::from_raw(dummy_raw2);

        q.retire(res1, 10);
        q.retire(res2, 15);

        assert!(q.entries[0].is_some());
        assert!(q.entries[1].is_some());

        // At frame 18: diff for res1 is 8 (18 - 10 <= 8), so kept
        q.drain(18);
        assert!(q.entries[0].is_some(), "Frame 10 entry must be preserved at frame 18");
        assert!(q.entries[1].is_some(), "Frame 15 entry must be preserved at frame 18");

        // At frame 19: diff for res1 is 9 (> 8), so dropped
        // We forget the inner pointer so Drop doesn't call Release on dummy
        let entry0 = q.entries[0].take().unwrap();
        std::mem::forget(entry0.tex);

        let entry1 = q.entries[1].take().unwrap();
        std::mem::forget(entry1.tex);
    }
}

// Pure Rust D3D12 Committed Resource Substitution Engine.
//
// Automatically intercepts multi-mip render targets (Output and Depth) during
// NVSDK_NGX_D3D12_EvaluateFeature and supplies dedicated single-mip D3D12 committed
// resources with ALLOW_UNORDERED_ACCESS, copying Subresource 0 and synchronizing
// barriers on the active command list.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

pub struct SubTex {
    pub tex: Option<ID3D12Resource>,
    pub width: u64,
    pub height: u32,
    pub fmt: DXGI_FORMAT,
    pub state: D3D12_RESOURCE_STATES,
}

impl SubTex {
    pub const fn new() -> Self {
        Self {
            tex: None,
            width: 0,
            height: 0,
            fmt: DXGI_FORMAT(0),
            state: D3D12_RESOURCE_STATE_COMMON,
        }
    }
}

pub struct RetiredEntry {
    pub tex: ID3D12Resource,
    pub frame: i64,
}

pub struct RetiredQueue {
    pub entries: [Option<RetiredEntry>; 8],
}

impl RetiredQueue {
    pub const fn new() -> Self {
        Self {
            entries: [None, None, None, None, None, None, None, None],
        }
    }

    pub fn retire(&mut self, tex: ID3D12Resource, frame: i64) {
        for slot in self.entries.iter_mut() {
            if slot.is_none() {
                *slot = Some(RetiredEntry { tex, frame });
                return;
            }
        }
        let mut oldest_idx = 0;
        let mut oldest_frame = i64::MAX;
        for (i, slot) in self.entries.iter().enumerate() {
            if let Some(ref e) = slot {
                if e.frame < oldest_frame {
                    oldest_frame = e.frame;
                    oldest_idx = i;
                }
            }
        }
        self.entries[oldest_idx] = Some(RetiredEntry { tex, frame });
    }

    pub fn drain(&mut self, current_frame: i64) {
        for slot in self.entries.iter_mut() {
            if let Some(ref e) = slot {
                if current_frame - e.frame > 8 {
                    *slot = None;
                }
            }
        }
    }
}

pub struct SubstitutionEngine {
    pub sub_out: SubTex,
    pub sub_depth: SubTex,
    pub retired: RetiredQueue,
}

impl SubstitutionEngine {
    pub const fn new() -> Self {
        Self {
            sub_out: SubTex::new(),
            sub_depth: SubTex::new(),
            retired: RetiredQueue::new(),
        }
    }
}

pub static ENGINE: Mutex<SubstitutionEngine> = Mutex::new(SubstitutionEngine::new());
pub static FRAME_COUNTER: AtomicI64 = AtomicI64::new(0);

// NGX Parameter virtual method bindings matching MSVC x64 ABI
// In MSVC, overloaded virtual functions are emitted in reverse declaration order:
// slot 1: Set(const char*, ID3D12Resource*) [0x08]
// slot 3: Set(const char*, int)              [0x18]
// slot 9: Get(const char*, ID3D12Resource**) [0x48]
// slot 11: Get(const char*, int*)            [0x58]
// slot 12: Get(const char*, unsigned int*)   [0x60]

pub unsafe fn ngx_param_get_d3d12_resource(
    params_ptr: *mut std::ffi::c_void,
    name: &str,
) -> Option<ID3D12Resource> {
    if params_ptr.is_null() {
        return None;
    }
    let vtbl = *(params_ptr as *mut *mut usize);
    if vtbl.is_null() {
        return None;
    }

    let get_fn_addr = *vtbl.add(9);
    if get_fn_addr == 0 {
        return None;
    }

    type PfnGetResource = unsafe extern "system" fn(
        *const std::ffi::c_void,
        *const std::os::raw::c_char,
        *mut *mut std::ffi::c_void,
    ) -> i32;
    let get_fn: PfnGetResource = std::mem::transmute(get_fn_addr);

    let c_name = std::ffi::CString::new(name).ok()?;
    let mut raw_out: *mut std::ffi::c_void = std::ptr::null_mut();
    let _ = get_fn(params_ptr, c_name.as_ptr(), &mut raw_out);

    if !raw_out.is_null() {
        // Wrap as ID3D12Resource without taking an extra reference (NGX does not AddRef in Get)
        Some(ID3D12Resource::from_raw(raw_out))
    } else {
        None
    }
}

pub unsafe fn ngx_param_set_d3d12_resource(
    params_ptr: *mut std::ffi::c_void,
    name: &str,
    res: Option<&ID3D12Resource>,
) {
    if params_ptr.is_null() {
        return;
    }
    let vtbl = *(params_ptr as *mut *mut usize);
    if vtbl.is_null() {
        return;
    }

    let set_fn_addr = *vtbl.add(1);
    if set_fn_addr == 0 {
        return;
    }

    type PfnSetResource = unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const std::os::raw::c_char,
        *mut std::ffi::c_void,
    );
    let set_fn: PfnSetResource = std::mem::transmute(set_fn_addr);

    if let Ok(c_name) = std::ffi::CString::new(name) {
        let raw_ptr = match res {
            Some(r) => r.as_raw(),
            None => std::ptr::null_mut(),
        };
        set_fn(params_ptr, c_name.as_ptr(), raw_ptr);
    }
}

pub unsafe fn fix_nr_quality_preset(params_ptr: *mut std::ffi::c_void) -> bool {
    if params_ptr.is_null() {
        return false;
    }
    let vtbl = *(params_ptr as *mut *mut usize);
    if vtbl.is_null() {
        return false;
    }

    let set_int_addr = *vtbl.add(3);
    let get_int_addr = *vtbl.add(11);
    let get_uint_addr = *vtbl.add(12);

    if set_int_addr == 0 || get_uint_addr == 0 || get_int_addr == 0 {
        return false;
    }

    type PfnSetInt = unsafe extern "system" fn(*mut std::ffi::c_void, *const std::os::raw::c_char, i32);
    type PfnGetUInt = unsafe extern "system" fn(*const std::ffi::c_void, *const std::os::raw::c_char, *mut u32) -> i32;
    type PfnGetInt = unsafe extern "system" fn(*const std::ffi::c_void, *const std::os::raw::c_char, *mut i32) -> i32;

    let set_int_fn: PfnSetInt = std::mem::transmute(set_int_addr);
    let get_uint_fn: PfnGetUInt = std::mem::transmute(get_uint_addr);
    let get_int_fn: PfnGetInt = std::mem::transmute(get_int_addr);

    let mut w = 0u32;
    let mut ow = 0u32;
    let mut q = -1i32;

    if let (Ok(c_w), Ok(c_ow), Ok(c_q)) = (
        std::ffi::CString::new("Width"),
        std::ffi::CString::new("OutWidth"),
        std::ffi::CString::new("PerfQualityValue"),
    ) {
        let _ = get_uint_fn(params_ptr, c_w.as_ptr(), &mut w);
        let _ = get_uint_fn(params_ptr, c_ow.as_ptr(), &mut ow);
        let _ = get_int_fn(params_ptr, c_q.as_ptr(), &mut q);

        if (w == ow || w == 0) && q != 5 {
            set_int_fn(params_ptr, c_q.as_ptr(), 5);
            return true;
        }
    }
    false
}

pub unsafe fn ensure_sub(
    sub: &mut SubTex,
    dev: &ID3D12Device,
    src_desc: &D3D12_RESOURCE_DESC,
    retired: &mut RetiredQueue,
    frame_no: i64,
    label: &str,
) -> bool {
    if let Some(ref _tex) = sub.tex {
        if sub.width == src_desc.Width && sub.height == src_desc.Height && sub.fmt == src_desc.Format {
            return true;
        }
    }

    if let Some(old_tex) = sub.tex.take() {
        retired.retire(old_tex, frame_no);
    }

    let hp = D3D12_HEAP_PROPERTIES {
        Type: D3D12_HEAP_TYPE_DEFAULT,
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 1,
        VisibleNodeMask: 1,
    };

    let mut d = *src_desc;
    d.MipLevels = 1;
    d.Flags = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    d.Layout = D3D12_TEXTURE_LAYOUT_UNKNOWN;

    let mut new_res: Option<ID3D12Resource> = None;
    let hr = dev.CreateCommittedResource(
        &hp,
        D3D12_HEAP_FLAG_NONE,
        &d,
        D3D12_RESOURCE_STATE_COMMON,
        None,
        &mut new_res,
    );

    if hr.is_err() || new_res.is_none() {
        crate::log::log(&format!("[dlss-mip-fix] {label}: failed to create 1-mip substitute: {:?}", hr));
        return false;
    }

    sub.tex = new_res;
    sub.width = src_desc.Width;
    sub.height = src_desc.Height;
    sub.fmt = src_desc.Format;
    sub.state = D3D12_RESOURCE_STATE_COMMON;
    crate::log::log(&format!(
        "[dlss-mip-fix] {label}: 1-mip substitute ready ({}x{} fmt={:?})",
        sub.width, sub.height, sub.fmt
    ));
    true
}

pub unsafe fn transition(
    list: &ID3D12GraphicsCommandList,
    res: &ID3D12Resource,
    from: D3D12_RESOURCE_STATES,
    to: D3D12_RESOURCE_STATES,
) {
    if from == to {
        return;
    }
    let mut b: D3D12_RESOURCE_BARRIER = std::mem::zeroed();
    b.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
    b.Anonymous.Transition = std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: std::mem::transmute_copy(res),
        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
        StateBefore: from,
        StateAfter: to,
    });
    list.ResourceBarrier(&[b]);
}

pub unsafe fn to_state(
    list: &ID3D12GraphicsCommandList,
    sub: &mut SubTex,
    target: D3D12_RESOURCE_STATES,
) {
    if let Some(ref tex) = sub.tex {
        if sub.state != target {
            transition(list, tex, sub.state, target);
            sub.state = target;
        }
    }
}

pub unsafe fn copy_mip0(
    list: &ID3D12GraphicsCommandList,
    dst: &ID3D12Resource,
    src: &ID3D12Resource,
) {
    let mut d: D3D12_TEXTURE_COPY_LOCATION = std::mem::zeroed();
    d.pResource = std::mem::transmute_copy(dst);
    d.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    d.Anonymous.SubresourceIndex = 0;

    let mut s: D3D12_TEXTURE_COPY_LOCATION = std::mem::zeroed();
    s.pResource = std::mem::transmute_copy(src);
    s.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    s.Anonymous.SubresourceIndex = 0;

    list.CopyTextureRegion(&d, 0, 0, 0, &s, None);
}

/// Substitution guard managing the lifecycle of D3D12 render targets for a single Evaluate call.
pub struct EvaluateSubstitutionGuard {
    pub params: *mut std::ffi::c_void,
    pub cmd_list_ptr: *mut std::ffi::c_void,
    pub orig_out: Option<ID3D12Resource>,
    pub orig_dep: Option<ID3D12Resource>,
    pub did_out: bool,
    pub did_dep: bool,
}

impl EvaluateSubstitutionGuard {
    pub unsafe fn prepare(
        cmd_list_ptr: *mut std::ffi::c_void,
        params_ptr: *mut std::ffi::c_void,
        allowed: bool,
    ) -> Option<Self> {
        if cmd_list_ptr.is_null() || params_ptr.is_null() || !allowed {
            return None;
        }

        let orig_out = ngx_param_get_d3d12_resource(params_ptr, "Output");
        let orig_dep = ngx_param_get_d3d12_resource(params_ptr, "Depth");

        let desc_out = orig_out.as_ref().map(|r| r.GetDesc());
        let desc_dep = orig_dep.as_ref().map(|r| r.GetDesc());

        let out_mipped = desc_out.as_ref().map_or(false, |d| d.MipLevels > 1);
        let dep_mipped = desc_dep.as_ref().map_or(false, |d| d.MipLevels > 1);

        if !out_mipped && !dep_mipped {
            if let Some(r) = orig_out { std::mem::forget(r); }
            if let Some(r) = orig_dep { std::mem::forget(r); }
            return None;
        }

        let list = ID3D12GraphicsCommandList::from_raw(cmd_list_ptr);
        let frame_no = FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);

        let mut engine_guard = ENGINE.lock().unwrap();
        let engine = &mut *engine_guard;
        engine.retired.drain(frame_no);

        let mut did_out = false;
        let mut did_dep = false;

        // Query device from original output resource
        let mut dev: Option<ID3D12Device> = None;
        if let Some(ref r) = orig_out {
            let _ = r.GetDevice(&mut dev);
        }

        if let Some(ref device) = dev {
            // 1. Substitute Output
            if out_mipped {
                if let (Some(ref orig), Some(ref desc)) = (&orig_out, &desc_out) {
                    if ensure_sub(&mut engine.sub_out, device, desc, &mut engine.retired, frame_no, "Output") {
                        // Preload output: seed substitute with game's existing frame content
                        transition(&list, orig, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
                        to_state(&list, &mut engine.sub_out, D3D12_RESOURCE_STATE_COPY_DEST);
                        copy_mip0(&list, engine.sub_out.tex.as_ref().unwrap(), orig);
                        transition(&list, orig, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);

                        to_state(&list, &mut engine.sub_out, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
                        ngx_param_set_d3d12_resource(params_ptr, "Output", engine.sub_out.tex.as_ref());
                        did_out = true;
                    }
                }
            }

            // 2. Substitute Depth
            if dep_mipped {
                if let (Some(ref orig), Some(ref desc)) = (&orig_dep, &desc_dep) {
                    if ensure_sub(&mut engine.sub_depth, device, desc, &mut engine.retired, frame_no, "Depth") {
                        transition(&list, orig, D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_SOURCE);
                        to_state(&list, &mut engine.sub_depth, D3D12_RESOURCE_STATE_COPY_DEST);
                        copy_mip0(&list, engine.sub_depth.tex.as_ref().unwrap(), orig);
                        to_state(&list, &mut engine.sub_depth, D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE);
                        transition(&list, orig, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE);

                        ngx_param_set_d3d12_resource(params_ptr, "Depth", engine.sub_depth.tex.as_ref());
                        did_dep = true;
                    }
                }
            }
        }

        std::mem::forget(list);

        Some(Self {
            params: params_ptr,
            cmd_list_ptr,
            orig_out,
            orig_dep,
            did_out,
            did_dep,
        })
    }

    pub unsafe fn complete(self) {
        if !self.did_out && !self.did_dep {
            return;
        }

        let list = ID3D12GraphicsCommandList::from_raw(self.cmd_list_ptr);
        let mut engine = ENGINE.lock().unwrap();

        // 1. Copyback Output from substitute back to game's original texture
        if self.did_out {
            if let Some(ref orig) = self.orig_out {
                to_state(&list, &mut engine.sub_out, D3D12_RESOURCE_STATE_COPY_SOURCE);
                if let Some(ref sub_res) = engine.sub_out.tex {
                    transition(&list, orig, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_DEST);
                    copy_mip0(&list, orig, sub_res);
                    transition(&list, orig, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
                }
                to_state(&list, &mut engine.sub_out, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
                ngx_param_set_d3d12_resource(self.params, "Output", Some(orig));
            }
        }

        // 2. Restore Depth pointer
        if self.did_dep {
            if let Some(ref orig) = self.orig_dep {
                ngx_param_set_d3d12_resource(self.params, "Depth", Some(orig));
            }
        }

        std::mem::forget(list);

        if let Some(r) = self.orig_out { std::mem::forget(r); }
        if let Some(r) = self.orig_dep { std::mem::forget(r); }
    }
}


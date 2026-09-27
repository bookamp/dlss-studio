use dlss_mip_addon::hook::{is_detoured, Hook};

#[test]
fn test_is_detoured_x64_rip_relative_jmp() {
    let mut code = [0u8; 14];
    code[0] = 0xFF;
    code[1] = 0x25;
    code[2] = 0x00;
    code[3] = 0x00;
    code[4] = 0x00;
    code[5] = 0x00;
    // target address: 0x00007FFE12345678
    code[6..14].copy_from_slice(&0x00007FFE12345678usize.to_ne_bytes());

    assert!(is_detoured(&code));
}

#[test]
fn test_is_detoured_rel32_jmp() {
    let code = [0xE9, 0x40, 0x12, 0x00, 0x00];
    assert!(is_detoured(&code));
}

#[test]
fn test_is_detoured_short_jmp() {
    let code = [0xEB, 0x2A];
    assert!(is_detoured(&code));
}

#[test]
fn test_is_detoured_mov_rax_jmp_rax() {
    let mut code = [0u8; 12];
    code[0] = 0x48;
    code[1] = 0xB8;
    code[2..10].copy_from_slice(&0x00007FFE99F38740usize.to_ne_bytes());
    code[10] = 0xFF;
    code[11] = 0xE0;

    assert!(is_detoured(&code));
}

#[test]
fn test_is_detoured_rejects_standard_function_prologues() {
    // Standard MSVC x64 function prologue: mov [rsp+8], rbx
    let prologue_mov_rsp = [0x48, 0x89, 0x5C, 0x24, 0x08, 0x48, 0x89, 0x74, 0x24, 0x10];
    assert!(!is_detoured(&prologue_mov_rsp));

    // push rbx; sub rsp, 32
    let prologue_push_sub = [0x40, 0x53, 0x48, 0x83, 0xEC, 0x20];
    assert!(!is_detoured(&prologue_push_sub));

    // push rbp; mov rbp, rsp
    let prologue_frame_ptr = [0x55, 0x48, 0x8B, 0xEC];
    assert!(!is_detoured(&prologue_frame_ptr));

    // Empty slice
    assert!(!is_detoured(&[]));
}

#[test]
fn test_build_patch_produces_correct_x64_bytes() {
    let target_addr: usize = 0x00007FFEC997E690;
    let patch = Hook::build_patch(target_addr);

    assert_eq!(&patch[0..6], &[0xFF, 0x25, 0x00, 0x00, 0x00, 0x00]);
    let extracted_addr = usize::from_ne_bytes(patch[6..14].try_into().unwrap());
    assert_eq!(extracted_addr, target_addr);
}

#[test]
fn test_pin_module_valid_function() {
    unsafe {
        dlss_mip_addon::pin_module(test_pin_module_valid_function as *const ());
    }
}

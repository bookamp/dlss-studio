use dlss_mip_addon::detector::{is_ngx_runtime_module, select_detoured_module, ModuleCandidate};

#[test]
fn test_select_detoured_module_finds_detoured_nvngx_over_unhooked_streamline_proxy() {
    let candidates = vec![
        ModuleCandidate {
            name: r"C:\Games\A Plague Tale Requiem\Content\sl.common.dll".to_string(),
            base_address: 0x00007FFE80000000,
            eval_entry_bytes: vec![0x48, 0x89, 0x5C, 0x24, 0x08, 0x57, 0x48, 0x83, 0xEC, 0x20],
        },
        ModuleCandidate {
            name: r"C:\Windows\System32\DriverStore\_nvngx.dll".to_string(),
            base_address: 0x00007FFE99F30000,
            eval_entry_bytes: vec![
                0xFF, 0x25, 0x00, 0x00, 0x00, 0x00, 0x90, 0xE6, 0x97, 0xC9, 0xFE, 0x7F, 0x00,
                0x00,
            ],
        },
        ModuleCandidate {
            name: r"C:\Games\A Plague Tale Requiem\Content\nvngx_dlss.dll".to_string(),
            base_address: 0x00007FFE70000000,
            eval_entry_bytes: vec![0x40, 0x53, 0x48, 0x83, 0xEC, 0x20],
        },
    ];

    let selected = select_detoured_module(&candidates).expect("Should find detoured module");
    assert_eq!(selected.base_address, 0x00007FFE99F30000);
    assert!(selected.name.contains("_nvngx.dll"));
}

#[test]
fn test_is_ngx_runtime_module() {
    assert!(is_ngx_runtime_module(r"C:\Windows\System32\DriverStore\_nvngx.dll"));
    assert!(is_ngx_runtime_module(r"C:\Windows\System32\nvngx.dll"));
    assert!(is_ngx_runtime_module(r"_nvngx.dll"));
    assert!(is_ngx_runtime_module(r"nvngx.dll"));

    assert!(!is_ngx_runtime_module(r"C:\Games\Requiem\nvngx_dlss.dll"));
    assert!(!is_ngx_runtime_module(r"C:\Games\Requiem\nvngx_dlssg.dll"));
    assert!(!is_ngx_runtime_module(r"C:\Games\Requiem\nvngx_dlssnr.dll"));
    assert!(!is_ngx_runtime_module(r"C:\Games\Requiem\sl.common.dll"));
    assert!(!is_ngx_runtime_module(r"C:\Games\Requiem\sl.interposer.dll"));
}

#[test]
fn test_select_detoured_module_strictly_ignores_virgin_e9_in_nvngx_dlss() {
    let mut candidates = vec![
        ModuleCandidate {
            name: r"D:\Games\Requiem\nvngx_dlss.dll".to_string(),
            base_address: 0x00007FFCC8EE0000,
            eval_entry_bytes: vec![0xE9, 0x6B, 0x0C, 0x00, 0x00, 0xCC, 0xCC],
        },
        ModuleCandidate {
            name: r"C:\Windows\System32\DriverStore\_nvngx.dll".to_string(),
            base_address: 0x00007FFCC9D70000,
            eval_entry_bytes: vec![0x48, 0x89, 0x5C, 0x24, 0x08, 0x48, 0x89],
        },
    ];

    assert!(
        select_detoured_module(&candidates).is_none(),
        "Must NOT hook nvngx_dlss.dll even though its export begins with E9"
    );

    candidates[1].eval_entry_bytes = vec![
        0xFF, 0x25, 0x00, 0x00, 0x00, 0x00, 0x90, 0xE6, 0x02, 0x46, 0xFD, 0x7F, 0x00, 0x00,
    ];

    let selected = select_detoured_module(&candidates)
        .expect("Must lock onto _nvngx.dll once RenoDX installs its detour");
    assert_eq!(selected.base_address, 0x00007FFCC9D70000);
    assert_eq!(selected.name, r"C:\Windows\System32\DriverStore\_nvngx.dll");
}

#[test]
fn test_select_detoured_module_handles_resonance_single_driver_module() {
    let candidates = vec![
        ModuleCandidate {
            name: r"C:\Windows\System32\DriverStore\_nvngx.dll".to_string(),
            base_address: 0x00007FFE8C490000,
            eval_entry_bytes: vec![
                0xFF, 0x25, 0x00, 0x00, 0x00, 0x00, 0x50, 0x1A, 0xF2, 0x21, 0xFF, 0x7F, 0x00,
                0x00,
            ],
        },
        ModuleCandidate {
            name: r"E:\Games\Resonance\nvngx_dlss.dll".to_string(),
            base_address: 0x00007FFE50000000,
            eval_entry_bytes: vec![0x48, 0x89, 0x5C, 0x24, 0x08],
        },
    ];

    let selected = select_detoured_module(&candidates).expect("Should find detoured module");
    assert_eq!(selected.base_address, 0x00007FFE8C490000);
    assert!(selected.name.contains("_nvngx.dll"));
}

#[test]
fn test_select_detoured_module_returns_none_when_no_module_detoured() {
    let candidates = vec![
        ModuleCandidate {
            name: r"C:\Windows\System32\DriverStore\_nvngx.dll".to_string(),
            base_address: 0x00007FFE99F30000,
            eval_entry_bytes: vec![0x48, 0x89, 0x5C, 0x24, 0x08, 0x57],
        },
    ];

    assert!(select_detoured_module(&candidates).is_none());
}

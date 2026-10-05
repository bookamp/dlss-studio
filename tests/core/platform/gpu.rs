use dlss_studio::core::gpu::*;

#[test]
fn test_is_ada_lovelace_by_device_id() {
    // NVIDIA Vendor ID is 0x10de
    // RTX 4090 Device ID is 0x2684
    assert!(is_ada_lovelace("Generic Display", 0x10de, 0x2684));
    // RTX 4080 Device ID is 0x2704
    assert!(is_ada_lovelace("Generic Display", 0x10de, 0x2704));

    // RTX 3080 Device ID is 0x2206 (not in 0x2600..=0x28ff)
    assert!(!is_ada_lovelace("RTX 3080", 0x10de, 0x2206));
}

#[test]
fn test_is_ada_lovelace_by_name() {
    assert!(is_ada_lovelace("NVIDIA GeForce RTX 4090", 0, 0));
    assert!(is_ada_lovelace("NVIDIA GeForce RTX 4070 Ti", 0, 0));
    assert!(is_ada_lovelace("NVIDIA RTX 4000 Ada Generation", 0, 0));

    // Older RTX 4000 (Turing)
    assert!(!is_ada_lovelace("NVIDIA Quadro RTX 4000 Turing", 0, 0));
    // AMD Radeon RX 7900 XTX
    assert!(!is_ada_lovelace("AMD Radeon RX 7900 XTX", 0x1002, 0x744c));
}

#[test]
fn test_detect_gpus_runs() {
    let gpus = detect_gpus();
    // On systems with a dedicated GPU, gpus will have entries; in any case it executes cleanly
    for gpu in &gpus {
        assert_ne!(gpu.vendor_id, 0x1414, "Basic render driver must be ignored");
        assert!(!gpu.name.is_empty());
    }
}


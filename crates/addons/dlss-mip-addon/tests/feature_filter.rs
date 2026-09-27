use dlss_mip_addon::{remember_feature, should_substitute};

#[test]
fn test_feature_filtering_substitutes_super_sampling_and_skips_neural_and_frame_generation() {
    let handle_ss = 0x1111 as *mut std::ffi::c_void;
    let handle_rr = 0x2222 as *mut std::ffi::c_void;
    let handle_fg = 0x3333 as *mut std::ffi::c_void;
    let handle_nr = 0x4444 as *mut std::ffi::c_void;
    let handle_unknown = 0x5555 as *mut std::ffi::c_void;

    remember_feature(handle_ss, 1);  // SuperSampling
    remember_feature(handle_rr, 11); // RayReconstruction
    remember_feature(handle_fg, 10); // FrameGeneration
    remember_feature(handle_nr, 18); // NeuralRendering

    assert_eq!(should_substitute(handle_ss), true, "SuperSampling must be substituted");
    assert_eq!(should_substitute(handle_rr), true, "RayReconstruction must be substituted");
    assert_eq!(should_substitute(handle_fg), false, "FrameGeneration must be skipped");
    assert_eq!(should_substitute(handle_nr), false, "NeuralRendering must be skipped");
    assert_eq!(should_substitute(handle_unknown), true, "Unknown handle must default to substitutable");
}

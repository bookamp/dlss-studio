use dlss_studio::core::payloads::scoring::*;

#[test]
fn test_score_optiscaler_dir_weights() {
    let score_plain = score_optiscaler_dir("OptiScaler_0.7.0");
    assert!(score_plain > 0);

    let score_higher = score_optiscaler_dir("OptiScaler_0.8.0");
    assert!(score_higher > score_plain);

    let score_dlssnr = score_optiscaler_dir("OptiScaler_0.7.0_dlssnr");
    assert!(score_dlssnr > score_higher);

    let score_mfg = score_optiscaler_dir("OptiScaler_0.7.0_rtx40mfg");
    assert!(score_mfg > score_dlssnr);
}

#[test]
fn test_score_optiscaler_dir_semver() {
    let score_patch = score_optiscaler_dir("OptiScaler_0.7.1");
    let score_base = score_optiscaler_dir("OptiScaler_0.7.0");
    assert!(score_patch > score_base);
}

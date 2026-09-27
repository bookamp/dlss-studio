use dlss_studio::core::APP_VERSION;

#[test]
fn test_app_version_matches_package() {
    assert_eq!(APP_VERSION, env!("CARGO_PKG_VERSION"));
    assert!(!APP_VERSION.is_empty());
}

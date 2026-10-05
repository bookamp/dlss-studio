use dlss_studio::core::addons::{
    catalog::*,
    ALL_REFERENCED_ADDONS,
};

#[test]
fn test_addon_id_all_have_metadata() {
    assert_eq!(ALL_REFERENCED_ADDONS.len(), 9);
    for id in ALL_REFERENCED_ADDONS {
        let meta = id.metadata();
        assert_eq!(meta.id, *id);
        assert!(!meta.name.is_empty());
        assert!(!meta.description_key.is_empty());
        assert!(!meta.url.is_empty());
        assert!(!meta.target_filename.is_empty());
    }
}

#[test]
fn test_addon_urls_and_hashes_structure() {
    assert!(FEEDER_ARCHIVE_URL.starts_with("https://"));
    assert_eq!(FEEDER_ARCHIVE_SHA256.len(), 64);

    assert!(MFG_UNLOCK_URL.starts_with("https://"));
    assert_eq!(MFG_UNLOCK_SHA256.len(), 64);

    assert!(DGVOODOO_URL.starts_with("https://"));
    assert_eq!(DGVOODOO_SHA256.len(), 64);

    assert!(RESHADE_SETUP_URL.starts_with("https://"));
    assert_eq!(RESHADE_SETUP_SHA256.len(), 64);
}

#[tokio::test]
async fn test_fallback_resolvers_return_valid_urls() {
    let (url, tag) = resolve_latest_dashdogy_mfg().await;
    assert!(url.starts_with("https://"));
    assert!(!tag.is_empty());

    let (opti_url, opti_tag) = resolve_latest_optiscaler_mfg().await;
    assert!(opti_url.starts_with("https://"));
    assert!(!opti_tag.is_empty());
}

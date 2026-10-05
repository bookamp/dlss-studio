pub mod cache;
pub mod catalog;
pub mod installer;

use dlss_studio::core::addons::*;

#[test]
fn test_addons_root_exports() {
    assert_eq!(ALL_REFERENCED_ADDONS.len(), 9);
    for id in ALL_REFERENCED_ADDONS {
        assert!(!id.name().is_empty());
        assert!(!id.description_key().is_empty());
        assert!(!id.url().is_empty());
    }
}

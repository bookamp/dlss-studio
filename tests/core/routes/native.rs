use dlss_studio::core::routes::native::*;
use dlss_studio::core::routes::Route;

#[test]
fn test_native_route_metadata() {
    let route = NativeRoute;
    assert_eq!(route.id(), "native");
    assert_eq!(route.display_name(), "DLSS 5 Direct (RenoDX)");
    assert_eq!(route.backend(), "reshade");
}

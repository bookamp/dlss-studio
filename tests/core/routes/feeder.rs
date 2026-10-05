use dlss_studio::core::routes::feeder::*;
use dlss_studio::core::routes::Route;

#[test]
fn test_feeder_route_metadata() {
    let route = FeederRoute;
    assert_eq!(route.id(), "feeder");
    assert_eq!(route.display_name(), "DLSS 5 Feeder");
    assert_eq!(route.backend(), "reshade");
}

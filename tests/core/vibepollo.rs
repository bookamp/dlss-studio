use dlss_studio::core::vibepollo::{
    ensure_vibepollo_cover_art, DEFAULT_APPS_JSON, DLSS_STUDIO_APP_NAME, DLSS_STUDIO_UUID,
};
use std::fs;

#[test]
fn test_apps_json_manipulation_mock() {
    let initial_json = serde_json::json!({
        "apps": [
            {
                "name": "Desktop",
                "image-path": "desktop.png",
                "uuid": "11111111-2222-3333-4444-555555555555"
            }
        ],
        "env": {},
        "version": 2
    });

    let mut doc = initial_json.clone();
    let apps = doc.get_mut("apps").unwrap().as_array_mut().unwrap();

    // Register DLSS Studio
    let entry = serde_json::json!({
        "name": DLSS_STUDIO_APP_NAME,
        "detached": [ "\"C:\\Path\\dlss-studio.exe\" --big-picture" ],
        "working-dir": "C:\\Path",
        "image-path": "C:\\Path\\poster.png",
        "uuid": DLSS_STUDIO_UUID
    });
    apps.push(entry);

    assert_eq!(apps.len(), 2);
    assert!(apps.iter().any(|a| a.get("name").and_then(|n| n.as_str()) == Some("DLSS Studio")));

    // Unregister DLSS Studio
    apps.retain(|a| a.get("name").and_then(|n| n.as_str()) != Some("DLSS Studio"));
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].get("name").and_then(|n| n.as_str()), Some("Desktop"));
}

#[test]
fn test_constants_definitions() {
    assert_eq!(DLSS_STUDIO_APP_NAME, "DLSS Studio");
    assert!(DLSS_STUDIO_UUID.contains("DLSS5STUDIO2"));
    assert!(DEFAULT_APPS_JSON.contains("apps.json"));
}

#[test]
fn test_ensure_vibepollo_cover_art_updates_stale_content() {
    let art_dir = dlss_studio::core::state::get_appdata_dir().join("art");
    let _ = fs::create_dir_all(&art_dir);
    let poster_path = art_dir.join("vibepollo_poster_v2.png");

    // Write intentional dummy bytes to simulate stale older cover
    let dummy = b"STALE_OLD_COVER_DATA";
    let _ = fs::write(&poster_path, dummy);
    assert_eq!(fs::read(&poster_path).unwrap(), dummy);

    // Invoking ensure_vibepollo_cover_art should immediately refresh to embedded poster
    let res = ensure_vibepollo_cover_art();
    assert!(res.is_some());
    let updated = fs::read(&poster_path).unwrap();
    assert!(updated.len() > 50_000);
    assert_eq!(&updated[..8], b"\x89PNG\r\n\x1a\n");
}

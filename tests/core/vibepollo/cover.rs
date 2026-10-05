use dlss_studio::core::vibepollo::cover::*;
use std::fs;

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

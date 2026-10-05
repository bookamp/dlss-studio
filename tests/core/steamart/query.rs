use dlss_studio::core::steamart::query::*;

#[test]
fn test_clean_name_tags_and_brackets() {
    assert_eq!(clean_name("Cyberpunk 2077 [v2.12] (DODI Repack)"), "Cyberpunk 2077");
    assert_eq!(clean_name("The.Witcher.3.Wild.Hunt-FitGirl"), "The Witcher 3 Wild Hunt");
    assert_eq!(clean_name("Baldur's Gate 3 (ElAmigos)"), "Baldur's Gate 3");
    assert_eq!(clean_name("Hogwarts Legacy [CODEX]"), "Hogwarts Legacy");
    assert_eq!(clean_name("Starfield [FitGirl Repack]"), "Starfield");
}

#[test]
fn test_norm_title_and_scoring() {
    assert_eq!(norm_title("Cyberpunk 2077: Phantom Liberty"), "cyberpunk 2077 phantom liberty");
    let items = vec![
        StoreItem { id: 1091500, name: Some("Cyberpunk 2077".to_string()), item_type: Some("app".to_string()) },
        StoreItem { id: 2138330, name: Some("Cyberpunk 2077: Phantom Liberty".to_string()), item_type: Some("app".to_string()) },
        StoreItem { id: 9999999, name: Some("Cyberpunk Bonus Pack".to_string()), item_type: None },
    ];
    let best = pick_best(&items, "Cyberpunk 2077");
    assert!(best.is_some());
    assert_eq!(best.unwrap().id, 1091500);

    let best_dlc = pick_best(&items, "Phantom Liberty");
    assert!(best_dlc.is_some());
    assert_eq!(best_dlc.unwrap().id, 2138330);
}

#[test]
fn test_steam_candidate_ranking_with_editions() {
    let items = vec![
        StoreItem {
            id: 3669870,
            name: Some("CONTROL Resonant".to_string()),
            item_type: Some("app".to_string()),
        },
        StoreItem {
            id: 870780,
            name: Some("CONTROL Ultimate Edition".to_string()),
            item_type: Some("app".to_string()),
        },
    ];

    let best = pick_best(&items, "Control");
    assert!(best.is_some());
    assert_eq!(best.unwrap().id, 870780, "Edition titles like 'CONTROL Ultimate Edition' must be prioritized over unrelated spinoffs");
}

#[test]
fn test_url_encode_and_decode() {
    let raw = "Hello World+Test&Special=123";
    let encoded = url_encode(raw);
    let decoded = url_decode(&encoded);
    assert_eq!(decoded, raw);
}

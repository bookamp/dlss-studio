use dlss_studio::core::state::session_log::*;

#[test]
fn test_ago_localized_multilingual() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

    // 30 seconds ago
    let t_just_now = now_ms.saturating_sub(30 * 1000);
    assert_eq!(ago_localized("en", t_just_now), "just now");
    assert_eq!(ago_localized("ru", t_just_now), "только что");
    assert_eq!(ago_localized("de", t_just_now), "gerade eben");
    assert_eq!(ago_localized("zh", t_just_now), "刚刚");

    // 10 minutes ago
    let t_10m = now_ms.saturating_sub(10 * 60 * 1000);
    assert_eq!(ago_localized("en", t_10m), "10m ago");
    assert_eq!(ago_localized("ru", t_10m), "10 мин назад");
    assert_eq!(ago_localized("de", t_10m), "vor 10 Min.");
    assert_eq!(ago_localized("zh", t_10m), "10分钟前");

    // 2 hours ago
    let t_2h = now_ms.saturating_sub(2 * 3600 * 1000);
    assert_eq!(ago_localized("en", t_2h), "2h ago");
    assert_eq!(ago_localized("ru", t_2h), "2 ч назад");
    assert_eq!(ago_localized("de", t_2h), "vor 2 Std.");
    assert_eq!(ago_localized("zh", t_2h), "2小时前");

    // 1 day ago (yesterday)
    let t_yest = now_ms.saturating_sub(25 * 3600 * 1000);
    assert_eq!(ago_localized("en", t_yest), "yesterday");
    assert_eq!(ago_localized("ru", t_yest), "вчера");
    assert_eq!(ago_localized("de", t_yest), "gestern");
    assert_eq!(ago_localized("zh", t_yest), "昨天");

    // 3 days ago
    let t_3d = now_ms.saturating_sub(3 * 86400 * 1000);
    assert_eq!(ago_localized("en", t_3d), "3d ago");
    assert_eq!(ago_localized("ru", t_3d), "3 дн назад");
    assert_eq!(ago_localized("de", t_3d), "vor 3 Tagen");
    assert_eq!(ago_localized("zh", t_3d), "3天前");
}

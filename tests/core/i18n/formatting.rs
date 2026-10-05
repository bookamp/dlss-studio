use dlss_studio::core::i18n::*;
use dlss_studio::core::downloader::{format_download_progress, DownloadProgress, DownloadStage};

#[test]
fn test_format_log_entry_multilingual_substitution() {
    let raw = "@{log_library_loaded|7}";
    assert_eq!(format_log_entry("en", raw), "Library loaded: 7 games in cache");
    assert_eq!(format_log_entry("fr", raw), "Bibliothèque chargée : 7 jeux en cache");
    assert_eq!(format_log_entry("de", raw), "Bibliothek geladen: 7 Spiele im Cache");
    assert_eq!(format_log_entry("ar", raw), "تم تحميل المكتبة: 7 ألعاب في الذاكرة المؤقتة");
    assert_eq!(format_log_entry("zh", raw), "游戏库已加载：缓存中存在 7 款游戏");
    assert_eq!(format_log_entry("hi", raw), "लाइब्रेरी लोड हुई: 7 गेम कैश में");

    let raw_toggle = "@{log_addon_status|builtin:renodx|@log_addon_activated}";
    assert_eq!(format_log_entry("en", raw_toggle), "Add-on builtin:renodx activated");
    assert_eq!(format_log_entry("fr", raw_toggle), "Extension builtin:renodx activée");
    assert_eq!(format_log_entry("de", raw_toggle), "Erweiterung builtin:renodx aktiviert");

    let raw_restore = "@{log_restore_start|Left 4 Dead}";
    assert_eq!(format_log_entry("en", raw_restore), "Restoring vanilla files for Left 4 Dead...");
    assert_eq!(format_log_entry("de", raw_restore), "Originaldateien für Left 4 Dead wiederherstellen...");
    assert_eq!(format_log_entry("zh", raw_restore), "正在恢复 Left 4 Dead 的原始游戏文件...");

    let raw_restore_ok = "@{log_restore_success}";
    assert_eq!(format_log_entry("en", raw_restore_ok), "Original game files restored successfully");
    assert_eq!(format_log_entry("de", raw_restore_ok), "Originale Spieledateien erfolgreich wiederhergestellt");
    assert_eq!(format_log_entry("es", raw_restore_ok), "Archivos originales del juego restaurados con éxito");

    let plain = "Unformatted plain text diagnostic message";
    assert_eq!(format_log_entry("fr", plain), plain);

    assert_eq!(format_log_entry("en", "Ready"), "Ready");
    assert_eq!(format_log_entry("zh", "Ready"), "就绪");
    assert_eq!(format_log_entry("de", "Ready"), "Bereit");
    assert_eq!(format_log_entry("ja", "Ready"), "準備完了");
}

#[test]
fn test_download_progress_localization() {
    // 1. Checking stage
    let prog_check = DownloadProgress::checking();
    assert_eq!(format_download_progress("en", &prog_check), "Checking components...");
    assert_eq!(format_download_progress("de", &prog_check), "Komponenten werden überprüft...");
    assert_eq!(format_download_progress("fr", &prog_check), "Vérification des composants...");
    assert_eq!(format_download_progress("zh", &prog_check), "正在检查核心组件...");

    // 2. Connecting stage
    let mut prog_conn = DownloadProgress::default();
    prog_conn.component_name = "Streamline Runtime v2.14.1".to_string();
    prog_conn.stage = DownloadStage::Connecting;
    assert_eq!(format_download_progress("en", &prog_conn), "Connecting to Streamline Runtime v2.14.1...");
    assert_eq!(format_download_progress("de", &prog_conn), "Verbindung zu Streamline Runtime v2.14.1 wird hergestellt...");
    assert_eq!(format_download_progress("es", &prog_conn), "Conectando a Streamline Runtime v2.14.1...");

    // 3. Downloading stage with byte progress
    let mut prog_dl = DownloadProgress::default();
    prog_dl.component_name = "Streamline Runtime v2.14.1".to_string();
    prog_dl.downloaded_bytes = 106_731_930; // ~101.79 MB
    prog_dl.total_bytes = Some(144_021_914); // ~137.35 MB
    prog_dl.stage = DownloadStage::Downloading;
    assert_eq!(
        format_download_progress("en", &prog_dl),
        "Downloading Streamline Runtime v2.14.1... (101.79 MB / 137.35 MB)"
    );
    assert_eq!(
        format_download_progress("de", &prog_dl),
        "Streamline Runtime v2.14.1 wird heruntergeladen... (101.79 MB / 137.35 MB)"
    );
    assert_eq!(
        format_download_progress("fr", &prog_dl),
        "Téléchargement de Streamline Runtime v2.14.1... (101.79 MB / 137.35 MB)"
    );
    assert_eq!(
        format_download_progress("ja", &prog_dl),
        "Streamline Runtime v2.14.1 をダウンロード中... (101.79 MB / 137.35 MB)"
    );

    // 4. Verifying stage
    let mut prog_ver = DownloadProgress::default();
    prog_ver.stage = DownloadStage::Verifying;
    assert_eq!(format_download_progress("en", &prog_ver), "Verifying checksum...");
    assert_eq!(format_download_progress("de", &prog_ver), "Prüfsumme wird überprüft...");
    assert_eq!(format_download_progress("zh", &prog_ver), "正在校验完整性与哈希值...");

    // 5. Retrying stage
    let mut prog_retry = DownloadProgress::default();
    prog_retry.component_name = "ReShade 6.8.0".to_string();
    prog_retry.stage = DownloadStage::Retrying(2);
    assert_eq!(format_download_progress("en", &prog_retry), "Retrying ReShade 6.8.0 (attempt 2/3)...");
    assert_eq!(format_download_progress("de", &prog_retry), "ReShade 6.8.0 wird wiederholt (Versuch 2/3)...");
}

#[test]
fn test_detect_system_language_execution() {
    let lang = detect_system_language();
    assert!(!lang.is_empty());
}

#[test]
fn test_translate_advisory_reason_all_branches() {
    let reasons = [
        "32-bit Architecture detected",
        "DirectX 11 Interop Notice: Bridge",
        "Non-DirectX 12 / Missing Native Upscaler detected",
        "Mod-Deployed DLSS Detected on system",
        "Legacy / Non-DirectX API detected",
        "Missing Native DLSS: not found",
        "Non-DirectX 12 API: running under DirectX 11.",
        "DirectX 12 Required: running under Vulkan.",
        "DirectX 11 Limitation: no MFG",
        "Missing Native DLSS-G: stream missing",
        "Hardware Requirement: RTX 40 required",
        "Custom Unknown Reason",
    ];

    for r in &reasons {
        let en = translate_advisory_reason("en", r);
        assert!(!en.is_empty());
        let de = translate_advisory_reason("de", r);
        assert!(!de.is_empty());
    }
}

#[test]
fn test_translate_advisory_recommendation_all_branches() {
    let recs = [
        "Use DLSS 5 Feeder route to enable DLSS 5 Neural Rendering on DirectX 11.",
        "DLSS 5 Feeder route provides generic frame interception for this title.",
        "For titles without native DLSS-G, use Feeder.",
        "bridge DirectX 11 DLSS calls to D3D12 via OptiScaler.",
        "Switch executable to Vulkan for better results.",
        "Unknown Recommendation",
    ];

    for rec in &recs {
        let en = translate_advisory_recommendation("en", rec);
        assert!(!en.is_empty());
        let fr = translate_advisory_recommendation("fr", rec);
        assert!(!fr.is_empty());
    }
}

#[test]
fn test_t_params_multiple_substitutions() {
    let res = t_params("en", "status_found_games", &["10"]);
    assert!(!res.is_empty());
}

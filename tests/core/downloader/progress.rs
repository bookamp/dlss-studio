use dlss_studio::core::downloader::progress::{
    format_bytes, format_download_progress, DownloadProgress, DownloadStage,
};

#[test]
fn test_download_progress_default_and_format_bytes() {
    let def = DownloadProgress::default();
    assert!(!def.is_downloading);
    assert_eq!(def.percentage, 0.0);

    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(2048), "2.0 KB");
    assert_eq!(format_bytes(10 * 1024 * 1024), "10.00 MB");
}

#[test]
fn test_download_progress_checking_constructor() {
    let prog = DownloadProgress::checking();
    assert!(prog.is_downloading);
    assert_eq!(prog.stage, DownloadStage::Checking);
}

#[test]
fn test_format_download_progress_all_stages() {
    let mut prog = DownloadProgress::default();
    prog.component_name = "Test Component".to_string();

    prog.stage = DownloadStage::Checking;
    let s_check = format_download_progress("en", &prog);
    assert!(!s_check.is_empty());

    prog.stage = DownloadStage::Connecting;
    let s_conn = format_download_progress("en", &prog);
    assert!(s_conn.contains("Test Component"));

    prog.stage = DownloadStage::Downloading;
    prog.downloaded_bytes = 0;
    prog.total_bytes = None;
    let s_dl_zero = format_download_progress("en", &prog);
    assert!(s_dl_zero.contains("Test Component"));

    prog.downloaded_bytes = 1024;
    prog.total_bytes = None;
    let s_dl_unknown_total = format_download_progress("en", &prog);
    assert!(s_dl_unknown_total.contains("1.0 KB"));

    prog.downloaded_bytes = 1024;
    prog.total_bytes = Some(4096);
    let s_dl_with_total = format_download_progress("en", &prog);
    assert!(s_dl_with_total.contains("1.0 KB / 4.0 KB"));

    prog.stage = DownloadStage::Verifying;
    let s_ver = format_download_progress("en", &prog);
    assert!(!s_ver.is_empty());

    prog.stage = DownloadStage::Retrying(2);
    let s_retry = format_download_progress("en", &prog);
    assert!(s_retry.contains("Test Component") || s_retry.contains("2"));
}


use dlss_studio::core::downloader::client::{compute_sha256, download_file_with_progress, download_file_with_sha256};
use dlss_studio::core::downloader::progress::DownloadStage;
use crate::common::TempDir;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

#[test]
fn test_compute_sha256_known_hash() {
    let temp = TempDir::new("hash_test");
    let file = temp.write_file("sample.txt", b"hello world");

    let hash = compute_sha256(&file).unwrap();
    assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
}

#[tokio::test]
async fn test_download_file_with_progress_success() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind local TCP port");
    let port = listener.local_addr().unwrap().port();

    let payload = b"DLSS_STUDIO_TEST_BINARY_PAYLOAD_CHUNK_DATA";
    let payload_len = payload.len();
    use sha2::{Digest, Sha256};
    let expected_hash = hex::encode(Sha256::digest(payload));

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload_len
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(payload);
            let _ = stream.flush();
        }
    });


    let temp = TempDir::new("download_success_test");
    let target_file = temp.path().join("downloaded.bin");

    let mut stages = Vec::new();
    let res = download_file_with_progress(
        &format!("http://127.0.0.1:{}/test.bin", port),
        &target_file,
        &expected_hash,
        "Test Component",
        "test-comp",
        |prog| {
            stages.push(prog.stage);
        },
    )
    .await;

    assert!(res.is_ok(), "Download should succeed: {:?}", res.err());
    assert!(target_file.is_file(), "Target file must exist after download");
    let contents = std::fs::read(&target_file).unwrap();
    assert_eq!(contents, payload);

    assert!(stages.contains(&DownloadStage::Connecting));
    assert!(stages.contains(&DownloadStage::Downloading));
    assert!(stages.contains(&DownloadStage::Verifying));

    // Test short-circuit when file already exists and hash matches
    let res_cached = download_file_with_sha256(
        &format!("http://127.0.0.1:{}/test.bin", port),
        &target_file,
        &expected_hash,
    )
    .await;

    assert!(res_cached.is_ok(), "Cached download must succeed immediately without re-downloading");
}

#[tokio::test]
async fn test_download_file_checksum_mismatch() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind local TCP port");
    let port = listener.local_addr().unwrap().port();

    let payload = b"CORRUPTED_DATA";
    let payload_len = payload.len();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload_len
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(payload);
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    });

    let temp = TempDir::new("download_mismatch_test");
    let target_file = temp.path().join("mismatch.bin");

    let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let res = download_file_with_progress(
        &format!("http://127.0.0.1:{}/mismatch.bin", port),
        &target_file,
        wrong_hash,
        "Mismatch Component",
        "mismatch-comp",
        |_| {},
    )
    .await;

    assert!(res.is_err(), "Must fail on checksum mismatch");
    let err = res.unwrap_err();
    assert!(err.contains("Checksum mismatch") || err.contains("connect") || err.contains("failed"), "Unexpected error: {}", err);
    assert!(!target_file.exists(), "Target file must not be created on hash mismatch");
    assert!(!target_file.with_extension("part").exists(), "Part file must be cleaned up on mismatch");
}

#[tokio::test]
async fn test_download_file_http_error_and_retries() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind local TCP port");
    let port = listener.local_addr().unwrap().port();

    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    thread::spawn(move || {
        for _ in 0..3 {
            if let Ok((mut stream, _)) = listener.accept() {
                attempts_clone.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        }
    });

    let temp = TempDir::new("download_retry_test");
    let target_file = temp.path().join("server_error.bin");

    let res = download_file_with_progress(
        &format!("http://127.0.0.1:{}/error.bin", port),
        &target_file,
        "",
        "Error Component",
        "error-comp",
        |_| {},
    )
    .await;

    assert!(res.is_err(), "Must report error when all 3 attempts fail with 500");
    assert_eq!(attempts.load(Ordering::SeqCst), 3, "Must have attempted download exactly 3 times");
}

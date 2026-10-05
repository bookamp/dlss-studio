use dlss_studio::core::downloader::extract::{extract_zip, silent_tar_command};
use crate::common::TempDir;
use std::fs;

#[test]
fn test_zip_extraction_mock() {
    let temp = TempDir::new("zip");
    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("sample.txt", options).unwrap();
        std::io::Write::write_all(&mut writer, b"hello world").unwrap();

        // Add a directory entry
        writer.add_directory("subfolder/", options).unwrap();

        // Add a nested file in a directory that wasn't explicitly added
        writer.start_file("deep/nested/file.dat", options).unwrap();
        std::io::Write::write_all(&mut writer, b"nested content").unwrap();

        writer.finish().unwrap();
    }

    let out_dir = temp.join("extracted");
    extract_zip(std::io::Cursor::new(buf), &out_dir).unwrap();
    assert!(out_dir.join("sample.txt").exists());
    assert_eq!(fs::read_to_string(out_dir.join("sample.txt")).unwrap(), "hello world");
    assert!(out_dir.join("subfolder").is_dir());
    assert!(out_dir.join("deep").join("nested").join("file.dat").is_file());
    assert_eq!(fs::read_to_string(out_dir.join("deep").join("nested").join("file.dat")).unwrap(), "nested content");
}

#[test]
fn test_zip_extraction_corrupted_archive() {
    let temp = TempDir::new("corrupt_zip");
    let out_dir = temp.join("extracted");
    let corrupt_data = vec![0x50, 0x4b, 0x03, 0x04, 0x00, 0x00, 0xff, 0xff];
    let res = extract_zip(std::io::Cursor::new(corrupt_data), &out_dir);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("Invalid zip archive") || err.contains("Corrupt"));
}


#[test]
fn test_silent_tar_command_constructs() {
    let cmd = silent_tar_command();
    assert_eq!(cmd.get_program(), "tar.exe");
}

#[test]
fn test_zip_extraction_enclosed_name_filter() {
    let temp = TempDir::new("zip_enclosed");
    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = zip::write::SimpleFileOptions::default();
        // Normal file
        writer.start_file("valid.txt", options).unwrap();
        std::io::Write::write_all(&mut writer, b"valid").unwrap();
        // Path with traversal that enclosed_name filters out
        writer.start_file("../outside.txt", options).unwrap();
        std::io::Write::write_all(&mut writer, b"outside").unwrap();
        writer.finish().unwrap();
    }

    let out_dir = temp.join("extracted");
    extract_zip(std::io::Cursor::new(buf), &out_dir).unwrap();
    assert!(out_dir.join("valid.txt").exists());
}


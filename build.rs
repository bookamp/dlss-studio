fn main() {
    #[cfg(windows)]
    {
        // Ensure the Windows 10 SDK rc.exe path is present in PATH for winres
        if let Ok(path) = std::env::var("PATH") {
            let win_sdk_bin = r"D:\Windows Kits\10\bin\10.0.22621.0\x64";
            if std::path::Path::new(win_sdk_bin).exists() && !path.contains(win_sdk_bin) {
                std::env::set_var("PATH", format!("{};{}", win_sdk_bin, path));
            }
        }

        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", "DLSS 5 Studio - Ultra-low latency DLSS 5 & OptiScaler Manager");
        res.set("ProductName", "DLSS 5 Studio");
        res.set("OriginalFilename", "dlss-studio.exe");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resource: {}", e);
        }
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=assets/icon.png");
        println!("cargo:rerun-if-changed=assets/icon_64.rgba");
        println!("cargo:rerun-if-changed=assets/icon_32.rgba");
    }

    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| ".".to_string());

    // Compress embedded payloads to minimize binary footprint

    let css_src = std::path::Path::new("assets/style.css");
    if css_src.exists() {
        let raw = std::fs::read(css_src).expect("Failed to read style.css");
        let compressed = miniz_oxide::deflate::compress_to_vec(&raw, 10);
        let dest = std::path::Path::new(&out_dir).join("style.css.deflate");
        std::fs::write(&dest, &compressed).expect("Failed to write compressed style.css");
        println!("cargo:rerun-if-changed=assets/style.css");
    }

    let payload_dest = std::path::Path::new(&out_dir).join("installer_payload.bin");

    let mut chosen_exe: Option<std::path::PathBuf> = None;

    // 1. Prioritize clean release binary
    let release_exe = std::path::PathBuf::from("target/release/dlss-studio.exe");
    if release_exe.exists() {
        chosen_exe = Some(release_exe);
    } else if let Ok(entries) = std::fs::read_dir("target/release") {
        // 2. Check for versioned portable release binary in target/release
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with("dlss-studio") && name.contains("portable") && name.ends_with(".exe") {
                    chosen_exe = Some(path);
                    break;
                }
            }
        }
    }

    if chosen_exe.is_none() {
        if let Ok(entries) = std::fs::read_dir("dist") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name.starts_with("dlss-studio") && name.contains("portable") && name.ends_with(".exe") {
                        chosen_exe = Some(path);
                        break;
                    }
                }
            }
        }
    }

    // 3. Fallback to debug only if no release binary exists anywhere
    if chosen_exe.is_none() {
        let debug_exe = std::path::PathBuf::from("target/debug/dlss-studio.exe");
        if debug_exe.exists() {
            chosen_exe = Some(debug_exe);
        }
    }

    if let Some(src) = chosen_exe {
        let _ = std::fs::copy(src, &payload_dest);
    } else if !payload_dest.exists() {
        let _ = std::fs::write(&payload_dest, &[]);
    }
    println!("cargo:rerun-if-changed=target/release/dlss-studio.exe");
    println!("cargo:rerun-if-changed=target/debug/dlss-studio.exe");
}

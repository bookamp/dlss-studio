//! Pure Rust Icon & RGBA Generator for DLSS Studio
//! Reads assets/icon.png and derives:
//! - assets/icon.ico (7-level multi-resolution Windows PE icon)
//! - assets/icon_64.rgba (64x64 raw RGBA byte buffer for Tao window icon)
//! - assets/icon_32.rgba (32x32 raw RGBA byte buffer for native Win32 tray icon)

use std::fs::{self, File};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};

fn resolve_assets_dir() -> PathBuf {
    // 1. CARGO_MANIFEST_DIR if set
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest_dir).join("assets");
        if p.exists() {
            return p;
        }
    }

    // 2. Current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("assets");
        if p.exists() {
            return p;
        }
    }

    // 3. Current executable parent fallback
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("assets");
            if p.exists() {
                return p;
            }
            if let Some(grandparent) = parent.parent() {
                let p = grandparent.join("assets");
                if p.exists() {
                    return p;
                }
            }
        }
    }

    PathBuf::from("assets")
}

/// Encodes multiple PNG frames into a single multi-resolution Windows ICO file.
/// Standard Windows ICO format (Vista/7/8/10/11) with PNG streams.
fn write_multi_resolution_ico(path: &Path, frames: &[(u32, Vec<u8>)]) -> io::Result<()> {
    let mut file = File::create(path)?;
    let count = frames.len() as u16;

    // 1. Write ICO Header (6 bytes)
    file.write_all(&0u16.to_le_bytes())?; // Reserved (must be 0)
    file.write_all(&1u16.to_le_bytes())?; // Type (1 for ICO)
    file.write_all(&count.to_le_bytes())?; // Image Count

    // 2. Compute directory entry offsets (Header is 6 bytes, entries are 16 bytes each)
    let mut current_offset = 6u32 + (count as u32 * 16u32);

    for (dim, png_data) in frames {
        let w_byte = if *dim >= 256 { 0u8 } else { *dim as u8 };
        let h_byte = if *dim >= 256 { 0u8 } else { *dim as u8 };
        let size = png_data.len() as u32;

        file.write_all(&[w_byte])?;                 // Width
        file.write_all(&[h_byte])?;                 // Height
        file.write_all(&[0u8])?;                    // Color count (0 for 32bpp)
        file.write_all(&[0u8])?;                    // Reserved
        file.write_all(&1u16.to_le_bytes())?;       // Color planes (1)
        file.write_all(&32u16.to_le_bytes())?;      // Bit count (32bpp)
        file.write_all(&size.to_le_bytes())?;       // Size of image data
        file.write_all(&current_offset.to_le_bytes())?; // Offset

        current_offset += size;
    }

    // 3. Write each image frame payload
    for (_, png_data) in frames {
        file.write_all(png_data)?;
    }

    file.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let assets_dir = resolve_assets_dir();
    let master_png = assets_dir.join("icon.png");

    let args: Vec<String> = std::env::args().collect();

    // Check for named flags: --poster <path>, --icon <path>, --badge <path>
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--poster" => {
                if i + 1 < args.len() {
                    let poster_src = PathBuf::from(&args[i + 1]);
                    println!("[INFO] Processing Vibepollo poster from: {}", poster_src.display());
                    let img = image::open(&poster_src)?.to_rgba8();
                    let resized = image::imageops::resize(&img, 600, 900, FilterType::Lanczos3);
                    let target = assets_dir.join("vibepollo_poster.png");
                    resized.save_with_format(&target, ImageFormat::Png)?;
                    println!("[SUCCESS] Saved 600x900 poster to: {}", target.display());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--icon" => {
                if i + 1 < args.len() {
                    let icon_src = PathBuf::from(&args[i + 1]);
                    println!("[INFO] Importing master icon from: {}", icon_src.display());
                    let img = image::open(&icon_src)?.to_rgba8();
                    let normalized = image::imageops::resize(&img, 512, 512, FilterType::Lanczos3);
                    normalized.save_with_format(&master_png, ImageFormat::Png)?;
                    println!("[SUCCESS] Master icon saved to: {}", master_png.display());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            arg if !arg.starts_with("--") => {
                // Positional icon source argument fallback
                let src_path = PathBuf::from(arg);
                if src_path.exists() {
                    println!("[INFO] Importing source image from: {}", src_path.display());
                    let src_img = image::open(&src_path)?.to_rgba8();
                    let normalized = image::imageops::resize(&src_img, 512, 512, FilterType::Lanczos3);
                    normalized.save_with_format(&master_png, ImageFormat::Png)?;
                    println!("[SUCCESS] Normalized master icon saved to: {} (512x512)", master_png.display());
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    if !master_png.exists() {
        eprintln!("[ERROR] Master icon not found at: {}", master_png.display());
        std::process::exit(1);
    }

    println!("[INFO] Reading master icon from: {}", master_png.display());
    let master_img = image::open(&master_png)?.to_rgba8();

    // 1. Generate multi-resolution Windows ICO (256, 128, 64, 48, 32, 24, 16)
    let sizes = [256u32, 128, 64, 48, 32, 24, 16];
    let mut ico_frames: Vec<(u32, Vec<u8>)> = Vec::new();

    for &size in &sizes {
        let resized = image::imageops::resize(
            &master_img,
            size,
            size,
            FilterType::Lanczos3,
        );

        let mut png_buf = Vec::new();
        let mut cursor = Cursor::new(&mut png_buf);
        resized.write_to(&mut cursor, ImageFormat::Png)?;
        ico_frames.push((size, png_buf));
    }

    let ico_path = assets_dir.join("icon.ico");
    write_multi_resolution_ico(&ico_path, &ico_frames)?;
    println!(
        "[SUCCESS] Wrote {} ({} mip-levels: {:?})",
        ico_path.display(),
        sizes.len(),
        sizes
    );

    // 2. Generate raw RGBA buffers for Tao window and native Win32 tray icons
    let raw_64: RgbaImage = image::imageops::resize(
        &master_img,
        64,
        64,
        FilterType::Lanczos3,
    );
    let raw_64_bytes = raw_64.into_raw();
    let rgba_64_path = assets_dir.join("icon_64.rgba");
    fs::write(&rgba_64_path, &raw_64_bytes)?;
    println!(
        "[SUCCESS] Wrote {} ({} bytes)",
        rgba_64_path.display(),
        raw_64_bytes.len()
    );

    let raw_32: RgbaImage = image::imageops::resize(
        &master_img,
        32,
        32,
        FilterType::Lanczos3,
    );
    let raw_32_bytes = raw_32.into_raw();
    let rgba_32_path = assets_dir.join("icon_32.rgba");
    fs::write(&rgba_32_path, &raw_32_bytes)?;
    println!(
        "[SUCCESS] Wrote {} ({} bytes)",
        rgba_32_path.display(),
        raw_32_bytes.len()
    );

    println!("[SUCCESS] Pure Rust icon synchronization completed.");
    Ok(())
}

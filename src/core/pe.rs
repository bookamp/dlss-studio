use pelite::pe32::Pe as _;
use pelite::pe64::Pe as _;
use pelite::{FileMap, PeFile, Wrap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const IMAGE_FILE_LARGE_ADDRESS_AWARE: u16 = 0x0020;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeInfo {
    pub bitness: u32,
    pub is_laa: bool,
    pub version: Option<String>,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
}

pub fn inspect_pe<P: AsRef<Path>>(path: P) -> Option<PeInfo> {
    let map = FileMap::open(path.as_ref()).ok()?;
    let file = PeFile::from_bytes(map.as_ref()).ok()?;

    let (bitness, is_laa, imports, exports) = match file {
        Wrap::T32(pe32) => {
            let mut list = Vec::new();
            if let Ok(import_dir) = pe32.imports() {
                for desc in import_dir {
                    if let Ok(dll_name) = desc.dll_name() {
                        list.push(dll_name.to_str().unwrap_or("").to_lowercase());
                    }
                }
            }
            let mut exp_list = Vec::new();
            if let Ok(exports) = pe32.exports() {
                if let Ok(by) = exports.by() {
                    for (name_res, _) in by.iter_names() {
                        if let Ok(name) = name_res {
                            exp_list.push(name.to_str().unwrap_or("").to_string());
                        }
                    }
                }
            }
            let is_laa = (pe32.file_header().Characteristics & IMAGE_FILE_LARGE_ADDRESS_AWARE) != 0;
            (32, is_laa, list, exp_list)
        }
        Wrap::T64(pe64) => {
            let mut list = Vec::new();
            if let Ok(import_dir) = pe64.imports() {
                for desc in import_dir {
                    if let Ok(dll_name) = desc.dll_name() {
                        list.push(dll_name.to_str().unwrap_or("").to_lowercase());
                    }
                }
            }
            let mut exp_list = Vec::new();
            if let Ok(exports) = pe64.exports() {
                if let Ok(by) = exports.by() {
                    for (name_res, _) in by.iter_names() {
                        if let Ok(name) = name_res {
                            exp_list.push(name.to_str().unwrap_or("").to_string());
                        }
                    }
                }
            }
            (64, true, list, exp_list)
        }
    };

    let version = get_version_info(&file);

    Some(PeInfo {
        bitness,
        is_laa,
        version,
        imports,
        exports,
    })
}

/// Checks whether an executable file has the IMAGE_FILE_LARGE_ADDRESS_AWARE (0x0020) flag set.
pub fn is_large_address_aware<P: AsRef<Path>>(path: P) -> bool {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut dos_header = [0u8; 64];
    if file.read_exact(&mut dos_header).is_err() {
        return false;
    }
    if dos_header[0] != b'M' || dos_header[1] != b'Z' {
        return false;
    }
    let e_lfanew = u32::from_le_bytes(dos_header[0x3C..0x40].try_into().unwrap()) as u64;
    let char_offset = e_lfanew + 22;
    if file.seek(SeekFrom::Start(char_offset)).is_err() {
        return false;
    }
    let mut char_bytes = [0u8; 2];
    if file.read_exact(&mut char_bytes).is_err() {
        return false;
    }
    let characteristics = u16::from_le_bytes(char_bytes);
    (characteristics & IMAGE_FILE_LARGE_ADDRESS_AWARE) != 0
}

/// Sets or unsets the IMAGE_FILE_LARGE_ADDRESS_AWARE (0x0020) flag in a PE executable's header in-place.
pub fn set_large_address_aware<P: AsRef<Path>>(path: P, enable: bool) -> std::io::Result<bool> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let path_ref = path.as_ref();
    if let Ok(meta) = std::fs::metadata(path_ref) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path_ref, perms);
        }
    }

    let mut file = OpenOptions::new().read(true).write(true).open(path_ref)?;
    let mut dos_header = [0u8; 64];
    file.read_exact(&mut dos_header)?;
    if dos_header[0] != b'M' || dos_header[1] != b'Z' {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Not an MZ binary"));
    }
    let e_lfanew = u32::from_le_bytes(dos_header[0x3C..0x40].try_into().unwrap()) as u64;
    let char_offset = e_lfanew + 22;
    file.seek(SeekFrom::Start(char_offset))?;
    let mut char_bytes = [0u8; 2];
    file.read_exact(&mut char_bytes)?;
    let current_char = u16::from_le_bytes(char_bytes);
    let new_char = if enable {
        current_char | IMAGE_FILE_LARGE_ADDRESS_AWARE
    } else {
        current_char & !IMAGE_FILE_LARGE_ADDRESS_AWARE
    };

    if new_char != current_char {
        file.seek(SeekFrom::Start(char_offset))?;
        file.write_all(&new_char.to_le_bytes())?;
        file.flush()?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn get_version_info(file: &PeFile) -> Option<String> {
    let resources = match file {
        Wrap::T32(pe32) => pe32.resources().ok()?,
        Wrap::T64(pe64) => pe64.resources().ok()?,
    };
    let version_info = resources.version_info().ok()?;

    if let Some(fixed) = version_info.fixed() {
        let fv = &fixed.dwFileVersion;
        if fv.Major > 0 || fv.Minor > 0 || fv.Patch > 0 || fv.Build > 0 {
            return Some(format!("{}.{}.{}.{}", fv.Major, fv.Minor, fv.Patch, fv.Build));
        }
    }

    if let Some(&lang) = version_info.translation().first() {
        if let Some(val) = version_info.value(lang, "FileVersion") {
            return Some(val.to_string());
        }
        if let Some(val) = version_info.value(lang, "ProductVersion") {
            return Some(val.to_string());
        }
    }

    None
}

#[inline(always)]
fn fast_find(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    let first = needle[0];
    let rest = &needle[1..];
    let rest_len = rest.len();
    let max_idx = haystack.len() - needle.len();

    let mut i = 0;
    while i <= max_idx {
        if haystack[i] == first {
            if &haystack[i + 1..i + 1 + rest_len] == rest {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Scans mapped or streamed bytes for Latin1 string markers with zero unnecessary heap allocations.
pub fn find_markers<P: AsRef<Path>>(path: P, markers: &[&str]) -> Vec<String> {
    let p = path.as_ref();
    // Fast path: memory map the file directly so the OS handles paging with zero heap buffers
    if let Ok(map) = FileMap::open(p) {
        let bytes = map.as_ref();
        let scan_limit = bytes.len().min(128 * 1024 * 1024);
        let view = &bytes[..scan_limit];
        let mut found = Vec::new();
        for &m in markers {
            if fast_find(view, m.as_bytes()) {
                found.push(m.to_string());
            }
        }
        return found;
    }

    // Fallback: chunk-based streaming with a compact 256KB buffer (16x smaller than legacy 4MB)
    let mut file = match File::open(p) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let Ok(size) = file.metadata().map(|m| m.len()) else { return Vec::new(); };
    if size == 0 { return Vec::new(); }

    let scan_limit = size.min(128 * 1024 * 1024);
    let chunk_size = 256 * 1024;
    let overlap = 64;
    let mut buf = vec![0u8; chunk_size + overlap];
    let mut carry = 0;
    let mut pos = 0u64;
    let mut found = Vec::new();

    while pos < scan_limit {
        if file.seek(SeekFrom::Start(pos)).is_err() { break; }
        let to_read = (chunk_size as u64).min(scan_limit - pos) as usize;
        let read = match file.read(&mut buf[carry..carry + to_read]) {
            Ok(n) if n > 0 => n,
            _ => break,
        };
        let view = &buf[..carry + read];
        for &m in markers {
            if !found.iter().any(|x: &String| x == m) {
                if fast_find(view, m.as_bytes()) {
                    found.push(m.to_string());
                }
            }
        }
        if found.len() == markers.len() { break; }
        let view_len = view.len();
        carry = overlap.min(view_len);
        buf.copy_within(view_len - carry..view_len, 0);
        pos += read as u64;
    }

    found
}

pub fn is_reshade_dll<P: AsRef<Path>>(path: P) -> (bool, Option<String>, bool) {
    let p = path.as_ref();
    if !p.is_file() {
        return (false, None, false);
    }
    let pe = match inspect_pe(p) {
        Some(info) => info,
        None => return (false, None, false),
    };

    let mut mentions = false;
    let mut has_addon_support = false;

    if let Ok(map) = pelite::FileMap::open(p) {
        if let Ok(file) = pelite::PeFile::from_bytes(map.as_ref()) {
            let res = match file {
                pelite::Wrap::T32(pe32) => pe32.resources().ok(),
                pelite::Wrap::T64(pe64) => pe64.resources().ok(),
            };
            if let Some(resources) = res {
                if let Ok(vi) = resources.version_info() {
                    for &lang in vi.translation() {
                        for key in &["ProductName", "FileDescription", "CompanyName", "OriginalFilename"] {
                            if let Some(val) = vi.value(lang, key) {
                                if val.to_lowercase().contains("reshade") {
                                    mentions = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let markers = find_markers(p, &["Searching for add-ons", "ReShade"]);
    if markers.iter().any(|m| m == "Searching for add-ons") {
        has_addon_support = true;
        mentions = true;
    }
    if markers.iter().any(|m| m == "ReShade") {
        mentions = true;
    }

    (mentions, pe.version, has_addon_support)
}

pub fn is_optiscaler_or_proxy<P: AsRef<Path>>(path: P) -> bool {
    let p = path.as_ref();
    if !p.is_file() {
        return false;
    }

    if let Ok(map) = pelite::FileMap::open(p) {
        if let Ok(file) = pelite::PeFile::from_bytes(map.as_ref()) {
            let res = match file {
                pelite::Wrap::T32(pe32) => pe32.resources().ok(),
                pelite::Wrap::T64(pe64) => pe64.resources().ok(),
            };
            if let Some(resources) = res {
                if let Ok(vi) = resources.version_info() {
                    for &lang in vi.translation() {
                        for key in &["ProductName", "FileDescription", "CompanyName", "OriginalFilename", "InternalName"] {
                            if let Some(val) = vi.value(lang, key) {
                                let lower = val.to_lowercase();
                                if lower.contains("optiscaler") || lower.contains("nitec") || lower.contains("reshade") || lower.contains("rtxmfg") || lower.contains("dashdogy") || lower.contains("dgvoodoo") {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let markers = ["OptiScaler", "nitec", "ReShade", "DLSS-NR", "RTXMFG", "Dashdogy", "Streamline", "dgVoodoo"];
    !find_markers(p, &markers).is_empty()
}

/// Inspects an existing sl.interposer.dll to determine if it belongs to the legacy Streamline 1.x ABI.
/// Legacy 1.x titles (such as A Plague Tale: Requiem) rely on 7 export functions that NVIDIA
/// completely removed in Streamline 2.x: slGetFeatureSettings, slGetFeatureConfiguration, slGetHooks, etc.
pub fn is_legacy_streamline_1x<P: AsRef<Path>>(path: P) -> bool {
    let p = path.as_ref();
    if !p.is_file() {
        return false;
    }

    if let Some(info) = inspect_pe(p) {
        // Tier 1: Check PE exports for legacy signatures (100% deterministic)
        if info.exports.iter().any(|e| {
            e == "slGetFeatureSettings"
                || e == "slGetFeatureConfiguration"
                || e == "slGetHooks"
                || e == "slGetNumHooks"
                || e == "slIsFeatureEnabled"
                || e == "slSetFeatureConstants"
                || e == "slSetFeatureEnabled"
        }) {
            return true;
        }

        // Tier 2: Check PE file version (Major version < 2)
        if let Some(ref ver) = info.version {
            let digits: String = ver.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(major) = digits.parse::<u32>() {
                if major < 2 && major > 0 {
                    return true;
                }
            }
        }
    }

    // Tier 3: Binary string scan fallback for raw export names
    if let Ok(bytes) = std::fs::read(p) {
        if fast_find(&bytes, b"slGetFeatureSettings\0")
            || fast_find(&bytes, b"slGetFeatureConfiguration\0")
            || fast_find(&bytes, b"slGetHooks\0")
        {
            return true;
        }
    }

    false
}



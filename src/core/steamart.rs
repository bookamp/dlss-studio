#![allow(dead_code)]

use std::fs;
use std::path::Path;
use regex::Regex;
use serde::Deserialize;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use crate::core::state::get_appdata_dir;

#[derive(Debug, Clone, Deserialize)]
struct StoreSearchResult {
    items: Option<Vec<StoreItem>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoreItem {
    pub id: u64,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GameArt {
    pub appid: Option<u64>,
    pub cover_path: Option<String>,
    pub hero_path: Option<String>,
}

/// Normalizes and cleans game folder names by removing scene tags, repacks, and edition markers.
pub fn clean_name(name: &str) -> String {
    let re_brackets = Regex::new(r"\[[^\]]*\]|\([^)]*\)").unwrap();
    let re_tags = Regex::new(r"(?i)\b(repack|fitgirl|dodi|elamigos|codex|rune|empress|plaza|skidrow|multi\d*)\b").unwrap();
    let re_punct = Regex::new(r"[_\-—–:.]+").unwrap();
    let re_spaces = Regex::new(r"\s+").unwrap();

    let s = re_brackets.replace_all(name, " ");
    let s = re_tags.replace_all(&s, " ");
    let s = re_punct.replace_all(&s, " ");
    let s = re_spaces.replace_all(&s, " ");
    s.trim().to_string()
}

#[doc(hidden)]
pub fn norm_title(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Scores candidate Steam search items against query string.
pub fn score_item(item: &StoreItem, query: &str) -> i32 {
    if let Some(ref t) = item.item_type {
        if t != "app" {
            return -100;
        }
    }
    let q = norm_title(query);
    let q_words: std::collections::HashSet<&str> = q.split_whitespace().collect();
    let n = norm_title(item.name.as_deref().unwrap_or_default());
    let words: Vec<&str> = n.split_whitespace().collect();
    let shared = words.iter().filter(|w| q_words.contains(*w)).count() as i32;

    let mut score: i32 = 0;
    if n == q {
        score += 120;
    } else if n.starts_with(&q) || q.starts_with(&n) {
        score += 70;
    }
    if !q_words.is_empty() {
        score += (shared * 40) / (q_words.len() as i32);
    }

    let edition_words: std::collections::HashSet<&str> = [
        "edition", "ultimate", "deluxe", "definitive", "enhanced", "complete",
        "remastered", "goty", "year", "standard", "director", "directors", "cut"
    ].iter().cloned().collect();

    for w in &words {
        if !q_words.contains(w) {
            if edition_words.contains(w) {
                score += 5;
            } else {
                score -= 8;
            }
        }
    }

    score
}

/// Evaluates candidate Steam search rows and picks the highest scoring match.
#[doc(hidden)]
pub fn pick_best(items: &[StoreItem], query: &str) -> Option<StoreItem> {
    let mut best: Option<StoreItem> = None;
    let mut best_score: i32 = -1;

    for item in items {
        let score = score_item(item, query);
        if score > best_score {
            best_score = score;
            best = Some(item.clone());
        }
    }

    if best_score > 10 {
        best
    } else {
        None
    }
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub fn url_decode(s: &str) -> String {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(val);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Helper to read file and convert to base64 data URI (legacy fallback)
pub fn file_to_data_uri(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 500 {
        return None;
    }
    let encoded = BASE64_STANDARD.encode(&bytes);
    Some(format!("data:image/jpeg;base64,{}", encoded))
}

pub fn bytes_to_data_uri(bytes: &[u8]) -> String {
    let encoded = BASE64_STANDARD.encode(bytes);
    format!("data:image/jpeg;base64,{}", encoded)
}

/// Saves and optimizes an image file for use as game artwork.
/// Oversized images (> 400 KB or dimensions > 600x900) are downscaled to fit within 600x900.
pub fn optimize_and_save_cover_image(src_path: &Path, dst_path: &Path) -> std::io::Result<u64> {
    let src_ext = src_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
    let dst_ext = dst_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());

    // If source and destination format match, file is small (< 300 KB), and dimensions fit, fast copy
    if src_ext == dst_ext && src_path != dst_path {
        if let Ok(meta) = src_path.metadata() {
            if meta.len() < 300_000 {
                if let Ok((w, h)) = image::image_dimensions(src_path) {
                    if w <= 600 && h <= 900 {
                        return fs::copy(src_path, dst_path);
                    }
                }
            }
        }
    }

    if let Ok(img) = image::open(src_path) {
        let (w, h) = (img.width(), img.height());
        let resized = if w > 600 || h > 900 {
            img.resize(600, 900, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        let ext = dst_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
        let fmt = match ext.as_deref() {
            Some("webp") => image::ImageFormat::WebP,
            Some("png") => image::ImageFormat::Png,
            _ => image::ImageFormat::Jpeg,
        };

        let write_target = if src_path == dst_path {
            dst_path.with_extension("tmp_opt")
        } else {
            dst_path.to_path_buf()
        };

        if resized.save_with_format(&write_target, fmt).is_ok() {
            if write_target != dst_path {
                let _ = fs::rename(&write_target, dst_path);
            }
            if let Ok(m) = dst_path.metadata() {
                return Ok(m.len());
            }
        }
        if write_target != dst_path && write_target.exists() {
            let _ = fs::remove_file(&write_target);
        }
    }

    if src_path != dst_path {
        fs::copy(src_path, dst_path)
    } else {
        Ok(src_path.metadata().map(|m| m.len()).unwrap_or(0))
    }
}

/// Saves and optimizes raw image bytes (e.g. from user file picker) to dst_path.
pub fn optimize_and_save_cover_bytes(bytes: &[u8], dst_path: &Path) -> std::io::Result<u64> {
    if bytes.len() < 300_000 {
        fs::write(dst_path, bytes)?;
        return Ok(bytes.len() as u64);
    }

    if let Ok(img) = image::load_from_memory(bytes) {
        let (w, h) = (img.width(), img.height());
        let resized = if w > 600 || h > 900 {
            img.resize(600, 900, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        let ext = dst_path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
        let fmt = match ext.as_deref() {
            Some("webp") => image::ImageFormat::WebP,
            Some("png") => image::ImageFormat::Png,
            _ => image::ImageFormat::Jpeg,
        };

        if resized.save_with_format(dst_path, fmt).is_ok() {
            if let Ok(m) = dst_path.metadata() {
                return Ok(m.len());
            }
        }
    }

    fs::write(dst_path, bytes)?;
    Ok(bytes.len() as u64)
}

/// Helper to convert a local file to a lightweight HTTP URI served by Wry's custom desktop protocol
pub fn file_to_art_uri(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let art_dir = get_appdata_dir().join("art");
    let _ = fs::create_dir_all(&art_dir);

    // If already in art cache, reference filename directly
    if let Ok(rel) = path.strip_prefix(&art_dir) {
        let s = rel.to_string_lossy().replace('\\', "/");
        return Some(format!("http://dlss-art.localhost/art/{}", s));
    }

    // Otherwise, copy/optimize into art cache under safe key
    let key = key_for_dir(path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let cached = art_dir.join(format!("{}.{}", key, ext));
    if !cached.exists() {
        let _ = optimize_and_save_cover_image(path, &cached);
    } else if let Ok(meta) = cached.metadata() {
        // If the cached file is an oversized legacy uncompressed image (> 1 MB), optimize it in place
        if meta.len() > 1_000_000 {
            let _ = optimize_and_save_cover_image(&cached, &cached);
        }
    }
    Some(format!("http://dlss-art.localhost/art/{}.{}", key, ext))
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArtCleanupStats {
    pub files_scanned: usize,
    pub files_removed: usize,
    pub bytes_reclaimed: u64,
}

/// Extracts the underlying cached art filename from an art URI or path.
/// E.g. "http://dlss-art.localhost/art/f376bf8f7f1228c0.png" -> Some("f376bf8f7f1228c0.png")
/// "dlss-art://art/0123456789abcdef-cover.jpg" -> Some("0123456789abcdef-cover.jpg")
pub fn extract_art_filename(uri_or_path: &str) -> Option<String> {
    if uri_or_path.is_empty() || uri_or_path.starts_with("data:image/") {
        return None;
    }
    let s = uri_or_path.split('?').next().unwrap_or(uri_or_path);
    let s = s.split('#').next().unwrap_or(s);
    let trimmed = s.trim_end_matches(['/', '\\']);

    if let Some(pos) = trimmed.rfind("/art/") {
        let name = &trimmed[pos + 5..];
        let clean = name.trim_matches(['/', '\\']);
        if !clean.is_empty() && !clean.contains('/') && !clean.contains('\\') {
            return Some(clean.to_string());
        }
    }
    if let Some(pos) = trimmed.rfind("dlss-art://") {
        let name = &trimmed[pos + 11..];
        let clean = name.trim_start_matches("localhost/").trim_start_matches("art/").trim_matches(['/', '\\']);
        if !clean.is_empty() && !clean.contains('/') && !clean.contains('\\') {
            return Some(clean.to_string());
        }
    }

    let p = Path::new(trimmed);
    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Identifies and removes all artwork files in the art cache that do not belong
/// to any active or hidden game in the user's library, or static whitelisted assets.
pub fn cleanup_unused_art_in_dir<P: AsRef<Path>>(
    art_dir: &Path,
    active_games: &[crate::core::scan::GameEntry],
    hidden_paths: &[P],
    custom_posters: &std::collections::HashMap<String, String>,
) -> ArtCleanupStats {
    if !art_dir.is_dir() {
        return ArtCleanupStats::default();
    }

    // 1. Build set of valid directory hash keys
    let mut valid_keys = std::collections::HashSet::new();
    for g in active_games {
        valid_keys.insert(key_for_dir(&g.dir));
    }
    for p in hidden_paths {
        valid_keys.insert(key_for_dir(p.as_ref()));
    }

    // 2. Build set of explicit filenames referenced in posters or state
    let mut explicit_filenames = std::collections::HashSet::new();
    for g in active_games {
        if let Some(ref p) = g.poster {
            if let Some(name) = extract_art_filename(p) {
                explicit_filenames.insert(name.to_lowercase());
            }
        }
    }
    for (_, poster_val) in custom_posters {
        if let Some(name) = extract_art_filename(poster_val) {
            explicit_filenames.insert(name.to_lowercase());
        }
    }

    // 3. Protected static whitelist
    let protected_static = [
        "rust_on_iron.jpg",
        "vibepollo_poster_v2.png",
        "vibepollo_poster_v2.webp",
        "rust_dark.webp",
        "rust_light.webp",
    ];

    // 4. Iterate art/ and delete unreferenced files
    let mut stats = ArtCleanupStats::default();
    if let Ok(entries) = fs::read_dir(art_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            stats.files_scanned += 1;
            let filename = entry.file_name().to_string_lossy().to_string();
            let filename_lower = filename.to_lowercase();

            // Check if protected static asset
            if protected_static.iter().any(|&s| s.eq_ignore_ascii_case(&filename_lower)) {
                continue;
            }

            // Check if explicitly referenced filename
            if explicit_filenames.contains(&filename_lower) {
                continue;
            }

            // Check if filename starts with or matches any active game key
            let is_active_game_art = valid_keys.iter().any(|k| filename_lower.starts_with(k));
            if is_active_game_art {
                continue;
            }

            // Orphaned file!
            let file_size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if fs::remove_file(&path).is_ok() {
                stats.files_removed += 1;
                stats.bytes_reclaimed += file_size;
                crate::core::logger::info(
                    "art",
                    &format!("Purged orphaned artwork: {} (reclaimed {} KB)", filename, file_size / 1024),
                );
            }
        }
    }

    stats
}

/// Helper that runs cleanup against the application's configured art directory.
pub fn cleanup_unused_art<P: AsRef<Path>>(
    active_games: &[crate::core::scan::GameEntry],
    hidden_paths: &[P],
    custom_posters: &std::collections::HashMap<String, String>,
) -> ArtCleanupStats {
    let art_dir = get_appdata_dir().join("art");
    cleanup_unused_art_in_dir(&art_dir, active_games, hidden_paths, custom_posters)
}

/// Computes the total byte size of an art directory.
pub fn get_art_cache_size_in_dir(art_dir: &Path) -> u64 {
    if !art_dir.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(art_dir) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    total += m.len();
                }
            }
        }
    }
    total
}

/// Computes the total byte size of the application's configured art directory.
pub fn get_art_cache_size() -> u64 {
    get_art_cache_size_in_dir(&get_appdata_dir().join("art"))
}

/// Normalizes any poster URI (legacy dlss-art://, raw local file paths, or current scheme)
/// into a Wry-compatible http://dlss-art.localhost/art/... URL.
pub fn normalize_art_uri(uri: &str) -> String {
    if uri.starts_with("data:image/") {
        return uri.to_string();
    }
    if uri.starts_with("http://dlss-art.localhost/") {
        return uri.to_string();
    }
    if let Some(rest) = uri.strip_prefix("dlss-art://art/") {
        return format!("http://dlss-art.localhost/art/{}", rest.trim_start_matches('/'));
    }
    if let Some(rest) = uri.strip_prefix("dlss-art://") {
        let clean = rest
            .trim_start_matches("localhost/")
            .trim_start_matches("local/")
            .trim_start_matches("art/");
        return format!("http://dlss-art.localhost/art/{}", clean.trim_start_matches('/'));
    }
    if let Some(rest) = uri.strip_prefix("http://dlss-art.local/art/") {
        return format!("http://dlss-art.localhost/art/{}", rest.trim_start_matches('/'));
    }
    // If it's a raw filesystem path, convert it via file_to_art_uri
    let p = Path::new(uri);
    if p.is_file() {
        if let Some(art_url) = file_to_art_uri(p) {
            return art_url;
        }
    }
    uri.to_string()
}

static EMBEDDED_RUST_DARK: &[u8] = include_bytes!("../../assets/rust_dark.webp");
static EMBEDDED_RUST_LIGHT: &[u8] = include_bytes!("../../assets/rust_light.webp");

fn mime_from_extension(filename: &str) -> &'static str {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if ext.eq_ignore_ascii_case("webp") {
        "image/webp"
    } else if ext.eq_ignore_ascii_case("png") {
        "image/png"
    } else if ext.eq_ignore_ascii_case("svg") {
        "image/svg+xml"
    } else {
        "image/jpeg"
    }
}

/// Serves static application assets located in the `assets/` folder, falling back to compile-time
/// embedded binaries so standalone portable single-file executables remain 100% self-contained.
pub fn serve_static_asset(name: &str) -> Option<(String, Vec<u8>)> {
    let clean_name = name.split('?').next().unwrap_or(name).trim_start_matches('/');

    // 1. Try reading directly from disk if assets/ exists alongside cwd or exe
    let mut candidate_paths = vec![std::path::PathBuf::from("assets").join(clean_name)];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidate_paths.push(parent.join("assets").join(clean_name));
        }
    }

    for path in &candidate_paths {
        if path.is_file() {
            if let Ok(bytes) = fs::read(path) {
                let mime = mime_from_extension(clean_name);
                return Some((mime.to_string(), bytes));
            }
        }
    }

    // 2. Embedded compile-time fallback for standalone portable executable distribution
    match clean_name {
        "rust_dark.webp" => Some(("image/webp".to_string(), EMBEDDED_RUST_DARK.to_vec())),
        "rust_light.webp" => Some(("image/webp".to_string(), EMBEDDED_RUST_LIGHT.to_vec())),
        _ => None,
    }
}

/// Handles requests to the custom dlss-art protocol, reading cached posters directly from disk.
/// Accepts URLs from Wry's internal rewrite (dlss-art://localhost/art/...), direct HTTP (http://dlss-art.localhost/art/...),
/// or legacy (dlss-art://art/...), as well as static application assets (http://dlss-art.localhost/assets/...).
pub fn handle_art_request(uri: &str) -> Option<(String, Vec<u8>)> {
    let clean = uri.split('?').next().unwrap_or(uri);

    // 1. Static application assets (/assets/...)
    if let Some(idx) = clean.find("/assets/") {
        let asset_name = &clean[idx + 8..];
        return serve_static_asset(asset_name);
    }
    let without_proto = clean
        .trim_start_matches("dlss-art://")
        .trim_start_matches("dlss-art:")
        .trim_start_matches("http://dlss-art.localhost/")
        .trim_start_matches("http://dlss-art.local/");
    if let Some(stripped) = without_proto.strip_prefix("assets/") {
        return serve_static_asset(stripped);
    }

    // 2. Game art poster cache
    let art_dir = get_appdata_dir().join("art");

    let target_file = if let Some(idx) = uri.find("file=") {
        let raw = &uri[idx + 5..];
        let raw_file = raw.split('&').next().unwrap_or(raw);
        let decoded = url_decode(raw_file);
        std::path::PathBuf::from(decoded)
    } else {
        let filename = if let Some(idx) = clean.find("/art/") {
            &clean[idx + 5..]
        } else if let Some(stripped) = clean.strip_prefix("art/") {
            stripped
        } else {
            without_proto.trim_start_matches("localhost/").trim_start_matches('/')
        };
        art_dir.join(filename.trim_start_matches('/'))
    };

    if target_file.is_file() {
        if let Ok(bytes) = fs::read(&target_file) {
            let mime = if target_file.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("png")).unwrap_or(false) {
                "image/png"
            } else if target_file.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("webp")).unwrap_or(false) {
                "image/webp"
            } else {
                "image/jpeg"
            };
            return Some((mime.to_string(), bytes));
        }
    }
    None
}

fn split_camel_or_numbers(s: &str) -> String {
    let mut out = String::new();
    let mut prev_char: Option<char> = None;
    for c in s.chars() {
        if let Some(p) = prev_char {
            let is_lower_to_upper = p.is_lowercase() && c.is_uppercase();
            let is_letter_to_digit = p.is_alphabetic() && c.is_numeric();
            let is_digit_to_letter = p.is_numeric() && c.is_alphabetic();
            if is_lower_to_upper || is_letter_to_digit || is_digit_to_letter {
                out.push(' ');
            }
        }
        out.push(c);
        prev_char = Some(c);
    }
    out
}

async fn query_steam_api(client: &reqwest::Client, q: &str) -> Option<StoreSearchResult> {
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&cc=us&l=en",
        url_encode(q)
    );
    let res = client.get(&url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    res.json::<StoreSearchResult>().await.ok()
}

/// Searches Steam's public store endpoint and returns a ranked list of candidate AppIDs with names.
pub async fn search_steam_candidates(name: &str) -> Vec<(u64, String)> {
    let query = clean_name(name);
    if query.is_empty() {
        return Vec::new();
    }

    let client = match reqwest::Client::builder()
        .user_agent("DLSS5-Studio-Native/1.0")
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut candidates: Vec<(i32, u64, String)> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    if let Some(data) = query_steam_api(&client, &query).await {
        for item in data.items.unwrap_or_default() {
            let s = score_item(&item, &query);
            if s > 10 && seen_ids.insert(item.id) {
                candidates.push((s, item.id, item.name.unwrap_or_else(|| query.to_string())));
            }
        }
    }

    // Try secondary split query if query contains camelCase or numbers (e.g., Cyberpunk2077 -> Cyberpunk 2077)
    let alt_query = clean_name(&split_camel_or_numbers(&query));
    if alt_query != query && !alt_query.is_empty() {
        if let Some(data) = query_steam_api(&client, &alt_query).await {
            for item in data.items.unwrap_or_default() {
                let s = score_item(&item, &alt_query);
                if s > 10 && seen_ids.insert(item.id) {
                    candidates.push((s, item.id, item.name.unwrap_or_else(|| alt_query.to_string())));
                }
            }
        }
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().map(|(_, id, name)| (id, name)).collect()
}

/// Searches Steam's public store endpoint for a matching game title and returns its AppID.
pub async fn search_steam_appid(name: &str) -> Option<(u64, String)> {
    search_steam_candidates(name).await.into_iter().next()
}

/// Discovers any cached art in %APPDATA%\dlss-5-studio\art\<key>.*
pub fn find_cached_art(dir: &Path) -> Option<String> {
    let art_dir = get_appdata_dir().join("art");
    let key = key_for_dir(dir);
    let candidates = [
        format!("{}-cover.jpg", key),
        format!("{}.webp", key),
        format!("{}.jpg", key),
        format!("{}.png", key),
        format!("{}-cover.webp", key),
        format!("{}-cover.png", key),
        format!("{}-hero.jpg", key),
    ];
    for c in &candidates {
        let f = art_dir.join(c);
        if f.exists() && f.metadata().map(|m| m.len() > 2000).unwrap_or(false) {
            return Some(format!("http://dlss-art.localhost/art/{}", c));
        }
    }
    None
}

/// Checks directory for GOG gameId in goggame-*.info
pub fn find_gog_game_id(dir: &Path) -> Option<String> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_lowercase();
            if fname.starts_with("goggame-") && fname.ends_with(".info") {
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(gid) = val.get("gameId").and_then(|v| v.as_str()) {
                            return Some(gid.trim().to_string());
                        } else if let Some(gid_num) = val.get("gameId").and_then(|v| v.as_i64()) {
                            return Some(gid_num.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Downloads official GOG game artwork via GOG's public product API as a reliable fallback.
pub async fn download_gog_art(client: &reqwest::Client, game_id: &str, art_dir: &Path, key: &str) -> Option<String> {
    let url = format!("https://api.gog.com/products/{}", game_id);
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let val = resp.json::<serde_json::Value>().await.ok()?;
    let images = val.get("images")?;

    let cover_url = images.get("logo2x")
        .or_else(|| images.get("logo"))
        .or_else(|| images.get("background"))
        .and_then(|v| v.as_str())?;

    let full_cover_url = if cover_url.starts_with("//") {
        format!("https:{}", cover_url)
    } else {
        cover_url.to_string()
    };

    let cover_file = art_dir.join(format!("{}-cover.jpg", key));
    if let Ok(c_resp) = client.get(&full_cover_url).send().await {
        if c_resp.status().is_success() {
            if let Ok(bytes) = c_resp.bytes().await {
                if bytes.len() > 2000 {
                    let _ = fs::write(&cover_file, &bytes);
                    return Some(format!("http://dlss-art.localhost/art/{}-cover.jpg", key));
                }
            }
        }
    }
    None
}

/// Generates a filesystem-safe cache key for a game directory.
pub fn key_for_dir(dir: &Path) -> String {
    let s = dir.to_string_lossy().to_lowercase().replace('/', "\\");
    let mut hasher = sha2::Sha256::default();
    use sha2::Digest;
    hasher.update(s.as_bytes());
    hex::encode(&hasher.finalize()[..8])
}

/// Resolves or downloads the cover and hero background images for a game.
/// Caches files locally in %APPDATA%\dlss-5-studio\art\<key>-cover.jpg and <key>-hero.jpg.
/// Returns lightweight http://dlss-art.localhost/art/<key>-cover.jpg URIs so WebView2 streams directly from disk with zero base64 bloat.
pub async fn resolve_game_art(name: &str, dir: &Path, known_appid: Option<u64>) -> GameArt {
    let art_dir = get_appdata_dir().join("art");
    let _ = fs::create_dir_all(&art_dir);
    let key = key_for_dir(dir);

    if let Some(cached_cover) = find_cached_art(dir) {
        let hero_file = art_dir.join(format!("{}-hero.jpg", key));
        let hero_cached = if hero_file.exists() && hero_file.metadata().map(|m| m.len() > 2000).unwrap_or(false) {
            Some(format!("http://dlss-art.localhost/art/{}-hero.jpg", key))
        } else {
            None
        };
        return GameArt {
            appid: known_appid,
            cover_path: Some(cached_cover),
            hero_path: hero_cached,
        };
    }

    let client = reqwest::Client::builder()
        .user_agent("DLSS5-Studio-Native/1.0")
        .build()
        .unwrap_or_default();

    let cover_file = art_dir.join(format!("{}-cover.jpg", key));
    let hero_file = art_dir.join(format!("{}-hero.jpg", key));

    let candidate_ids = if let Some(id) = known_appid {
        vec![(id, name.to_string())]
    } else {
        search_steam_candidates(name).await
    };

    let mut resolved_appid = None;
    let mut downloaded_cover = None;
    let mut downloaded_hero = None;

    for (appid, _) in candidate_ids {
        // 1. Download Cover Poster (library_600x900.jpg)
        let cover_url = format!("https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{}/library_600x900.jpg", appid);
        if let Ok(resp) = client.get(&cover_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if bytes.len() > 2000 {
                        let _ = fs::write(&cover_file, &bytes);
                        downloaded_cover = Some(format!("http://dlss-art.localhost/art/{}-cover.jpg", key));
                    }
                }
            }
        }

        // 2. Download Hero Banner (library_hero.jpg or fallback header.jpg)
        let hero_url = format!("https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{}/library_hero.jpg", appid);
        if let Ok(resp) = client.get(&hero_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if bytes.len() > 2000 {
                        let _ = fs::write(&hero_file, &bytes);
                        downloaded_hero = Some(format!("http://dlss-art.localhost/art/{}-hero.jpg", key));
                    }
                }
            }
        }

        if downloaded_hero.is_none() {
            let header_url = format!("https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{}/header.jpg", appid);
            if let Ok(resp) = client.get(&header_url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        if bytes.len() > 2000 {
                            let _ = fs::write(&hero_file, &bytes);
                            downloaded_hero = Some(format!("http://dlss-art.localhost/art/{}-hero.jpg", key));
                        }
                    }
                }
            }
        }

        if downloaded_cover.is_some() || downloaded_hero.is_some() {
            resolved_appid = Some(appid);
            break;
        }
    }

    // 3. If Steam candidates failed or returned no artwork, check GOG product API fallback
    if downloaded_cover.is_none() && downloaded_hero.is_none() {
        if let Some(gid) = find_gog_game_id(dir) {
            if let Some(gog_art_uri) = download_gog_art(&client, &gid, &art_dir, &key).await {
                downloaded_cover = Some(gog_art_uri);
            }
        }
    }

    let final_cover = downloaded_cover.or_else(|| downloaded_hero.clone());
    GameArt {
        appid: resolved_appid,
        cover_path: final_cover,
        hero_path: downloaded_hero,
    }
}




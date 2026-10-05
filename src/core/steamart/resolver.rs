//! Steam and GOG online artwork resolution and cache retrieval.

use std::fs;
use std::path::Path;
use crate::core::state::get_appdata_dir;
use super::query::{clean_name, score_item, split_camel_or_numbers, url_encode, StoreSearchResult};
use super::image::key_for_dir;

#[derive(Debug, Clone, Default)]
pub struct GameArt {
    pub appid: Option<u64>,
    pub cover_path: Option<String>,
    pub hero_path: Option<String>,
}

pub async fn query_steam_api(client: &reqwest::Client, q: &str) -> Option<StoreSearchResult> {
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

//! Client optionnel Genius API & gestion locale du `.env` (SPECIFICATIONS.md §4.2 & Issue #12).
//!
//! - Lit `GENIUS_API_KEY` dans le fichier `.env` local (jamais commité) avec fallback env.
//! - Si la clé est absente ou vide : ignoré silencieusement sans aucune erreur.
//! - Ignore automatiquement les pistes `Instrumental` pour ne pas injecter de fausses paroles.
//! - Fournit la lecture / sauvegarde sûre de `GENIUS_API_KEY` depuis les Paramètres en
//!   préservant les autres variables du `.env` local (ex. `DISCOGS_TOKEN`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tauri::Manager;

use crate::models::ApiKeysSettings;
use crate::tagger::rules::{detect_protected_keyword, normalize_for_match, strip_feat_from_title};

pub const GENIUS_ENV_KEY: &str = "GENIUS_API_KEY";
pub const GENIUS_SEARCH_URL: &str = "https://api.genius.com/search";
pub const LRCLIB_GET_URL: &str = "https://lrclib.net/api/get";
const GENIUS_TIMEOUT_SECS: u64 = 10;
const LRCLIB_TIMEOUT_SECS: u64 = 8;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResolvedLyrics {
    pub plain_lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibResponse {
    instrumental: Option<bool>,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeniusSearchEnvelope {
    response: Option<GeniusSearchResponse>,
}

#[derive(Debug, Deserialize)]
struct GeniusSearchResponse {
    #[serde(default)]
    hits: Vec<GeniusHit>,
}

#[derive(Debug, Deserialize)]
struct GeniusHit {
    #[serde(rename = "type")]
    hit_type: Option<String>,
    result: GeniusSongResult,
}

#[derive(Debug, Deserialize)]
struct GeniusSongResult {
    title: Option<String>,
    url: Option<String>,
    primary_artist: Option<GeniusArtist>,
}

#[derive(Debug, Deserialize)]
struct GeniusArtist {
    name: Option<String>,
}

fn http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .user_agent("TagmasterDJ/0.1.0 (Open Source Local Music Tagger)")
                .timeout(Duration::from_secs(GENIUS_TIMEOUT_SECS))
                .build()
                .map_err(|err| format!("Impossible d'initialiser le client HTTP Genius : {err}"))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// Détermine le chemin du fichier `.env` local (racine du projet en dev, `app_config_dir` en prod).
pub fn resolve_env_file_path(app: Option<&tauri::AppHandle>) -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let direct = cwd.join(".env");
        if direct.is_file() {
            return direct;
        }

        if cwd.file_name().and_then(|n| n.to_str()) == Some("src-tauri")
            && let Some(parent) = cwd.parent()
        {
            let parent_env = parent.join(".env");
            if parent_env.is_file()
                || parent.join(".env.example").is_file()
                || parent.join("package.json").is_file()
            {
                return parent_env;
            }
        }

        if cwd.join("src-tauri").is_dir() {
            return direct;
        }
    }

    if let Some(handle) = app
        && let Ok(config_dir) = handle.path().app_config_dir()
    {
        return config_dir.join(".env");
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".env")
}

const ENV_PLACEHOLDERS: &[&str] = &[
    "METTRE_LA_CLE_API_GENIUS_ICI",
    "METTRE_LE_TOKEN_DISCOGS_ICI",
    "YOUR_API_KEY_HERE",
];

/// Lit la valeur d'une clé dans un fichier `.env` donné (retourne `None` si absente, vide ou repère non rempli).
pub fn read_env_key_from_file(path: &Path, key: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let line = trimmed.strip_prefix("export ").map(str::trim).unwrap_or(trimmed);
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }

        let mut value = v.trim();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = &value[1..value.len() - 1];
        }
        let final_val = value.trim();
        if final_val.is_empty() || ENV_PLACEHOLDERS.contains(&final_val) {
            return None;
        }
        return Some(final_val.to_string());
    }
    None
}

/// Écrit ou met à jour une clé dans un fichier `.env` local en préservant les autres lignes.
pub fn write_env_key_to_file(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let clean_value: String = value
        .chars()
        .filter(|c| *c != '\r' && *c != '\n')
        .collect::<String>()
        .trim()
        .to_string();

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Impossible de préparer le dossier du fichier .env ({}) : {err}",
                parent.display()
            )
        })?;
    }

    let formatted_line = format!("{key}={clean_value}");
    let mut lines: Vec<String> = if path.is_file() {
        std::fs::read_to_string(path)
            .map_err(|err| format!("Impossible de lire {} : {err}", path.display()))?
            .lines()
            .map(str::to_string)
            .collect()
    } else {
        vec![
            "# Clés API optionnelles pour Tagmaster DJ (ne jamais commiter .env)".to_string(),
        ]
    };

    let mut replaced = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let effective = trimmed.strip_prefix("export ").map(str::trim).unwrap_or(trimmed);
        if let Some((k, _)) = effective.split_once('=')
            && k.trim() == key
        {
            *line = formatted_line.clone();
            replaced = true;
            break;
        }
    }

    if !replaced {
        lines.push(formatted_line);
    }

    let mut output = lines.join("\n");
    output.push('\n');

    std::fs::write(path, output)
        .map_err(|err| format!("Impossible d'enregistrer {} : {err}", path.display()))
}

pub fn load_genius_api_key(app: Option<&tauri::AppHandle>) -> Option<String> {
    let env_path = resolve_env_file_path(app);
    read_env_key_from_file(&env_path, GENIUS_ENV_KEY).or_else(|| {
        std::env::var(GENIUS_ENV_KEY)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
}

pub fn get_api_keys_settings(app: &tauri::AppHandle) -> ApiKeysSettings {
    let env_path = resolve_env_file_path(Some(app));
    let key = read_env_key_from_file(&env_path, GENIUS_ENV_KEY)
        .or_else(|| {
            std::env::var(GENIUS_ENV_KEY)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
        .unwrap_or_default();

    ApiKeysSettings {
        has_genius_api_key: !key.is_empty(),
        genius_api_key: key,
        env_file_path: env_path.to_string_lossy().into_owned(),
    }
}

pub fn save_genius_api_key(
    app: &tauri::AppHandle,
    api_key: &str,
) -> Result<ApiKeysSettings, String> {
    let env_path = resolve_env_file_path(Some(app));
    write_env_key_to_file(&env_path, GENIUS_ENV_KEY, api_key)?;
    let stored = read_env_key_from_file(&env_path, GENIUS_ENV_KEY).unwrap_or_default();

    Ok(ApiKeysSettings {
        has_genius_api_key: !stored.is_empty(),
        genius_api_key: stored,
        env_file_path: env_path.to_string_lossy().into_owned(),
    })
}

/// Retourne `true` si la piste est marquée `Instrumental` (dans le titre ou le fichier source).
pub fn is_instrumental_track(title: &str, source_file_name: Option<&str>) -> bool {
    detect_protected_keyword(title) == Some("instrumental")
        || source_file_name.and_then(detect_protected_keyword) == Some("instrumental")
}

fn token_recall(expected: &str, candidate: &str) -> f32 {
    let exp = normalize_for_match(expected);
    let cand = normalize_for_match(candidate);
    if exp.is_empty() || cand.is_empty() {
        return 0.0;
    }
    if exp == cand {
        return 1.0;
    }

    let exp_tokens: Vec<&str> = exp.split_whitespace().collect();
    let cand_tokens: Vec<&str> = cand.split_whitespace().collect();
    let matched = exp_tokens
        .iter()
        .filter(|tok| cand_tokens.contains(tok))
        .count();
    let recall = matched as f32 / exp_tokens.len().max(1) as f32;
    let dice = (2.0 * matched as f32) / (exp_tokens.len() + cand_tokens.len()).max(1) as f32;
    recall.min(dice)
}

fn pick_best_genius_hit(hits: Vec<GeniusHit>, artist: &str, title: &str) -> Option<String> {
    let clean_title = strip_feat_from_title(title);
    let effective_title = if clean_title.is_empty() {
        title.to_string()
    } else {
        clean_title
    };

    let mut best: Option<(f32, String)> = None;

    for hit in hits {
        if hit
            .hit_type
            .as_deref()
            .map(|t| !t.eq_ignore_ascii_case("song"))
            .unwrap_or(false)
        {
            continue;
        }

        let Some(url) = hit
            .result
            .url
            .as_deref()
            .map(str::trim)
            .filter(|u| u.starts_with("http"))
        else {
            continue;
        };

        let hit_title = hit.result.title.as_deref().unwrap_or("");
        let hit_artist = hit
            .result
            .primary_artist
            .as_ref()
            .and_then(|a| a.name.as_deref())
            .unwrap_or("");

        let lower_hit = hit_title.to_ascii_lowercase();
        if lower_hit.contains("instrumental")
            || lower_hit.contains("karaoke")
            || lower_hit.contains("(translation)")
        {
            continue;
        }

        let stripped_hit_title = strip_feat_from_title(hit_title);
        let title_score = token_recall(&effective_title, hit_title)
            .max(token_recall(&effective_title, &stripped_hit_title));
        let artist_score = token_recall(artist, hit_artist);
        let total = title_score * 0.6 + artist_score * 0.4;

        if title_score >= 0.55 && artist_score >= 0.55 {
            match &best {
                Some((best_score, _)) if *best_score >= total => {}
                _ => best = Some((total, url.to_string())),
            }
        }
    }

    best.map(|(_, url)| url)
}

fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(amp_idx) = rest.find('&') {
        out.push_str(&rest[..amp_idx]);
        let after_amp = &rest[amp_idx..];
        if let Some(semi_rel) = after_amp.find(';')
            && semi_rel <= 12
        {
            let entity = &after_amp[1..semi_rel];
            let decoded_char = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                "rsquo" | "lsquo" => Some('\''),
                "ldquo" | "rdquo" => Some('"'),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                "hellip" => Some('…'),
                "eacute" => Some('é'),
                "egrave" => Some('è'),
                "ecirc" => Some('ê'),
                "euml" => Some('ë'),
                "agrave" => Some('à'),
                "aacute" => Some('á'),
                "acirc" => Some('â'),
                "ccedil" => Some('ç'),
                "ntilde" => Some('ñ'),
                "oacute" => Some('ó'),
                "ocirc" => Some('ô'),
                "iacute" => Some('í'),
                "icirc" => Some('î'),
                "ugrave" => Some('ù'),
                "uacute" => Some('ú'),
                "ucirc" => Some('û'),
                "uuml" => Some('ü'),
                s if s.starts_with("#x") || s.starts_with("#X") => {
                    u32::from_str_radix(&s[2..], 16).ok().and_then(char::from_u32)
                }
                s if s.starts_with('#') => s[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };

            if let Some(ch) = decoded_char {
                out.push(ch);
                rest = &after_amp[semi_rel + 1..];
                continue;
            }
        }

        out.push('&');
        rest = &rest[amp_idx + 1..];
    }

    out.push_str(rest);
    out
}

fn extract_lyrics_block_text(inner_html: &str) -> String {
    let mut text = String::with_capacity(inner_html.len());
    let lower = inner_html.to_ascii_lowercase();
    let mut idx = 0;

    while idx < inner_html.len() {
        let Some(rel_lt) = inner_html[idx..].find('<') else {
            text.push_str(&decode_html_entities(&inner_html[idx..]));
            break;
        };

        let tag_start = idx + rel_lt;
        if tag_start > idx {
            text.push_str(&decode_html_entities(&inner_html[idx..tag_start]));
        }

        let Some(rel_gt) = inner_html[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + rel_gt + 1;
        let tag_lower = &lower[tag_start..tag_end];

        if tag_lower.starts_with("<br") || tag_lower.starts_with("</p") {
            text.push('\n');
        }

        idx = tag_end;
    }

    text
}

/// Extrait les paroles brutes depuis le HTML d'une page Genius (`data-lyrics-container="true"`).
pub fn extract_lyrics_from_genius_html(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0;
    let mut containers: Vec<String> = Vec::new();

    while cursor < lower.len() {
        let Some(rel_div) = lower[cursor..].find("<div") else {
            break;
        };
        let div_start = cursor + rel_div;
        let Some(rel_gt) = lower[div_start..].find('>') else {
            break;
        };
        let open_tag_end = div_start + rel_gt + 1;
        let open_tag_lower = &lower[div_start..open_tag_end];

        if !open_tag_lower.contains("data-lyrics-container=\"true\"")
            && !open_tag_lower.contains("data-lyrics-container='true'")
        {
            cursor = open_tag_end;
            continue;
        }

        let mut scan = open_tag_end;
        let mut depth: usize = 1;
        let mut exclude_depth: Option<usize> = None;
        let mut collected = String::new();
        let mut chunk_start = open_tag_end;

        while scan < lower.len() && depth > 0 {
            let next_open = lower[scan..].find("<div");
            let next_close = lower[scan..].find("</div");

            let (is_open, tag_pos) = match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => (true, scan + o),
                (Some(o), None) => (true, scan + o),
                (_, Some(c)) => (false, scan + c),
                (None, None) => break,
            };

            let Some(rel_tag_gt) = lower[tag_pos..].find('>') else {
                break;
            };
            let tag_end = tag_pos + rel_tag_gt + 1;

            if exclude_depth.is_none() && tag_pos > chunk_start {
                collected.push_str(&extract_lyrics_block_text(&html[chunk_start..tag_pos]));
            }

            if is_open {
                let is_self_closing = lower[tag_pos..tag_end].trim_end().ends_with("/>");
                if !is_self_closing {
                    depth += 1;
                    if exclude_depth.is_none()
                        && (lower[tag_pos..tag_end]
                            .contains("data-exclude-from-selection=\"true\"")
                            || lower[tag_pos..tag_end]
                                .contains("data-exclude-from-selection='true'"))
                    {
                        exclude_depth = Some(depth);
                    }
                }
            } else {
                depth = depth.saturating_sub(1);
                if let Some(ex_d) = exclude_depth
                    && depth < ex_d
                {
                    exclude_depth = None;
                }
            }

            scan = tag_end;
            chunk_start = tag_end;
        }

        let trimmed = collected.trim();
        if !trimmed.is_empty() {
            containers.push(trimmed.to_string());
        }
        cursor = scan.max(open_tag_end);
    }

    if containers.is_empty() {
        return None;
    }

    let joined = containers.join("\n\n").replace("\r\n", "\n");
    let mut clean_lines: Vec<String> = Vec::new();
    let mut consecutive_blanks = 0;

    for raw_line in joined.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            consecutive_blanks += 1;
            if consecutive_blanks <= 1 && !clean_lines.is_empty() {
                clean_lines.push(String::new());
            }
        } else {
            consecutive_blanks = 0;
            clean_lines.push(line.to_string());
        }
    }

    let final_lyrics = clean_lines.join("\n").trim().to_string();
    if final_lyrics.is_empty() {
        None
    } else {
        Some(final_lyrics)
    }
}

/// Récupère les paroles via Genius API avec une clé explicite.
/// Retourne `Ok(None)` silencieusement si la clé est absente/vide ou si la piste est `Instrumental`.
pub async fn fetch_lyrics_with_key(
    artist: &str,
    title: &str,
    source_file_name: Option<&str>,
    api_key: Option<&str>,
) -> Result<Option<String>, String> {
    if is_instrumental_track(title, source_file_name) {
        return Ok(None);
    }

    let Some(token) = api_key.map(str::trim).filter(|k| !k.is_empty()) else {
        return Ok(None);
    };

    let clean_title = strip_feat_from_title(title);
    let query = format!("{artist} {clean_title}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        return Ok(None);
    }

    let client = http_client()?;
    let search_res = client
        .get(GENIUS_SEARCH_URL)
        .bearer_auth(token)
        .query(&[("q", query.as_str())])
        .send()
        .await
        .map_err(|err| format!("Erreur réseau Genius : {err}"))?;

    if !search_res.status().is_success() {
        return Err(format!("API Genius HTTP {}", search_res.status()));
    }

    let envelope: GeniusSearchEnvelope = search_res
        .json()
        .await
        .map_err(|err| format!("Réponse JSON Genius invalide : {err}"))?;

    let hits = envelope.response.map(|r| r.hits).unwrap_or_default();
    let Some(song_url) = pick_best_genius_hit(hits, artist, title) else {
        return Ok(None);
    };

    let page_res = client
        .get(&song_url)
        .send()
        .await
        .map_err(|err| format!("Erreur réseau page Genius : {err}"))?;

    if !page_res.status().is_success() {
        return Ok(None);
    }

    let html = page_res
        .text()
        .await
        .map_err(|err| format!("Impossible de lire la page Genius : {err}"))?;

    Ok(extract_lyrics_from_genius_html(&html))
}

/// Récupère les paroles Genius de manière tolérante aux pannes pour un export audio :
/// ne bloque jamais l'export en cas d'absence de clé, de piste instrumentale ou d'erreur réseau.
pub async fn fetch_lyrics_for_track(
    artist: &str,
    title: &str,
    source_file_name: Option<&str>,
    app: Option<&tauri::AppHandle>,
) -> Option<String> {
    if is_instrumental_track(title, source_file_name) {
        return None;
    }

    let key = load_genius_api_key(app)?;
    fetch_lyrics_with_key(artist, title, source_file_name, Some(&key))
        .await
        .ok()
        .flatten()
}

fn parse_lrc_timestamp(token: &str) -> Option<u32> {
    let trimmed = token.trim();
    let (mins_raw, secs_raw) = trimmed.split_once(':')?;
    if !mins_raw.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mins = mins_raw.parse::<u32>().ok()?;

    let (sec_part, frac_part) = match secs_raw.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (secs_raw, None),
    };
    if sec_part.is_empty() || !sec_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let secs = sec_part.parse::<u32>().ok()?;
    if secs >= 60 {
        return None;
    }

    let millis = if let Some(frac) = frac_part {
        if frac.is_empty() || !frac.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut padded = frac.chars().take(3).collect::<String>();
        while padded.len() < 3 {
            padded.push('0');
        }
        padded.parse::<u32>().ok()?
    } else {
        0
    };

    Some(mins * 60_000 + secs * 1_000 + millis)
}

fn strip_leading_lrc_timestamps(line: &str) -> (Vec<u32>, &str) {
    let mut rest = line.trim();
    let mut timestamps = Vec::new();

    while rest.starts_with('[') {
        let Some(close_idx) = rest.find(']') else {
            break;
        };
        let token = &rest[1..close_idx];
        if let Some(ms) = parse_lrc_timestamp(token) {
            timestamps.push(ms);
            rest = rest[close_idx + 1..].trim_start();
        } else {
            break;
        }
    }

    (timestamps, rest.trim())
}

/// Parse un texte LRC (`[mm:ss.xx] texte`) en paires `(millisecondes, ligne)` triées.
pub fn parse_lrc_lines(lrc: &str) -> Vec<(u32, String)> {
    let mut entries: Vec<(u32, String)> = Vec::new();

    for raw_line in lrc.lines() {
        let (timestamps, text) = strip_leading_lrc_timestamps(raw_line);
        if timestamps.is_empty() {
            continue;
        }
        for ts in timestamps {
            entries.push((ts, text.to_string()));
        }
    }

    entries.sort_by_key(|(ts, _)| *ts);
    entries
}

/// Extrait les paroles brutes lisibles à partir d'un fichier LRC horodaté.
pub fn strip_lrc_timestamps(lrc: &str) -> String {
    let mut clean_lines: Vec<String> = Vec::new();
    let mut blanks = 0;

    for raw_line in lrc.lines() {
        let trimmed = raw_line.trim();
        let (timestamps, text) = strip_leading_lrc_timestamps(trimmed);
        if timestamps.is_empty() && trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Ignore les métadonnées LRC ([ti:], [ar:], [al:], [length:], etc.)
            continue;
        }
        let line_text = if timestamps.is_empty() { trimmed } else { text };
        if line_text.is_empty() {
            blanks += 1;
            if blanks <= 1 && !clean_lines.is_empty() {
                clean_lines.push(String::new());
            }
        } else {
            blanks = 0;
            clean_lines.push(line_text.to_string());
        }
    }

    clean_lines.join("\n").trim().to_string()
}

async fn query_lrclib(
    artist: &str,
    track: &str,
    album: Option<&str>,
) -> Option<ResolvedLyrics> {
    let client = http_client().ok()?;
    let mut req = client
        .get(LRCLIB_GET_URL)
        .timeout(Duration::from_secs(LRCLIB_TIMEOUT_SECS))
        .query(&[("artist_name", artist), ("track_name", track)]);

    if let Some(alb) = album.map(str::trim).filter(|s| !s.is_empty()) {
        req = req.query(&[("album_name", alb)]);
    }

    let res = req.send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }

    let payload: LrclibResponse = res.json().await.ok()?;
    if payload.instrumental == Some(true) {
        return None;
    }

    let synced_lyrics = payload
        .synced_lyrics
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !parse_lrc_lines(s).is_empty());

    let plain_lyrics = payload
        .plain_lyrics
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            synced_lyrics
                .as_deref()
                .map(strip_lrc_timestamps)
                .filter(|s| !s.is_empty())
        });

    if synced_lyrics.is_none() && plain_lyrics.is_none() {
        None
    } else {
        Some(ResolvedLyrics {
            plain_lyrics,
            synced_lyrics,
        })
    }
}

/// Résout les paroles synchronisées temps réel (LRCLIB sans clé -> ID3v2 SYLT + .lrc)
/// puis complète les paroles brutes via Genius API (`GENIUS_API_KEY`) si nécessaire.
/// Toujours tolérant aux pannes et désactivé automatiquement pour les pistes `Instrumental`.
pub async fn resolve_lyrics_for_track(
    artist: &str,
    title: &str,
    album: Option<&str>,
    source_file_name: Option<&str>,
    app: Option<&tauri::AppHandle>,
) -> ResolvedLyrics {
    if is_instrumental_track(title, source_file_name) {
        return ResolvedLyrics::default();
    }

    let clean_title = strip_feat_from_title(title);
    let track_query = if clean_title.is_empty() {
        title.trim()
    } else {
        clean_title.as_str()
    };

    let mut resolved = if let Some(alb) = album.map(str::trim).filter(|s| !s.is_empty()) {
        match query_lrclib(artist, track_query, Some(alb)).await {
            Some(found) => found,
            None => query_lrclib(artist, track_query, None)
                .await
                .unwrap_or_default(),
        }
    } else {
        query_lrclib(artist, track_query, None)
            .await
            .unwrap_or_default()
    };

    if resolved.plain_lyrics.is_none() {
        resolved.plain_lyrics = fetch_lyrics_for_track(artist, title, source_file_name, app).await;
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn skips_silently_when_genius_api_key_is_missing_or_empty() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        assert_eq!(
            rt.block_on(fetch_lyrics_with_key("Shay", "T mimi", None, None)),
            Ok(None)
        );
        assert_eq!(
            rt.block_on(fetch_lyrics_with_key("Shay", "T mimi", None, Some("   "))),
            Ok(None)
        );
    }

    #[test]
    fn skips_instrumental_tracks_even_when_api_key_is_provided() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        assert!(is_instrumental_track("Agora Hills (Instrumental)", None));
        assert!(is_instrumental_track(
            "Agora Hills",
            Some("Doja Cat - Agora Hills Instrumental.mp3")
        ));
        assert!(!is_instrumental_track("Gatitas", Some("Bad Gyal - Gatitas.mp3")));

        // Retourne Ok(None) sans effectuer d'appel réseau malgré la présence d'une clé factice
        let res = rt.block_on(fetch_lyrics_with_key(
            "Doja Cat",
            "Agora Hills (Instrumental)",
            Some("Agora Hills Instrumental.mp3"),
            Some("fake-token-never-sent"),
        ));
        assert_eq!(res, Ok(None));
    }

    #[test]
    fn extracts_clean_lyrics_from_genius_html_and_ignores_contributor_header() {
        let html = r#"
        <html><body>
          <div data-lyrics-container="true" class="Lyrics__Container-sc-test">
            <div data-exclude-from-selection="true"><div>42 Contributors</div><div>Translations</div></div>
            [Verse 1]<br/>Hola qu&eacute; tal &amp; c&#x27;est parti<br/><i>La nuit tombe</i>
          </div>
          <div class="Lyrics__Container-sc-test" data-lyrics-container="true">
            [Chorus]<br/>Gatitas en la pista<br/>Hasta ma&ntilde;ana
          </div>
        </body></html>
        "#;

        let extracted = extract_lyrics_from_genius_html(html).expect("lyrics should be extracted");
        assert!(!extracted.contains("Contributors"));
        assert!(!extracted.contains("Translations"));
        assert!(extracted.contains("[Verse 1]\nHola qué tal & c'est parti\nLa nuit tombe"));
        assert!(extracted.contains("[Chorus]\nGatitas en la pista\nHasta mañana"));
    }

    #[test]
    fn reads_and_writes_genius_key_in_env_file_without_clobbering_other_keys() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock error")
            .as_nanos();
        let env_path = std::env::temp_dir().join(format!("tagmaster_dj_env_test_{nanos}.env"));

        std::fs::write(
            &env_path,
            "# Commentaire local\nGENIUS_API_KEY=METTRE_LA_CLE_API_GENIUS_ICI\nDISCOGS_TOKEN=discogs-keep-me\n",
        )
        .expect("write initial env");

        assert_eq!(read_env_key_from_file(&env_path, GENIUS_ENV_KEY), None);

        write_env_key_to_file(&env_path, GENIUS_ENV_KEY, "  my-secret-genius-key \n")
            .expect("write genius key");

        assert_eq!(
            read_env_key_from_file(&env_path, GENIUS_ENV_KEY).as_deref(),
            Some("my-secret-genius-key")
        );
        assert_eq!(
            read_env_key_from_file(&env_path, "DISCOGS_TOKEN").as_deref(),
            Some("discogs-keep-me")
        );

        let _ = std::fs::remove_file(&env_path);
    }

    #[test]
    fn parses_lrc_timestamps_for_sylt_and_strips_clean_unsynced_lyrics() {
        let lrc = "\
[ar:Bad Gyal]
[ti:Gatitas]
[00:12.34] Salgo, que la calle ya me espera
[00:16.5] [chorus]
[00:20.125][01:05.00] Refrain en temps réel
";
        let parsed = parse_lrc_lines(lrc);
        assert_eq!(
            parsed,
            vec![
                (12_340, "Salgo, que la calle ya me espera".to_string()),
                (16_500, "[chorus]".to_string()),
                (20_125, "Refrain en temps réel".to_string()),
                (65_000, "Refrain en temps réel".to_string()),
            ]
        );

        let plain = strip_lrc_timestamps(lrc);
        assert!(!plain.contains("[ar:"));
        assert!(!plain.contains("[00:12.34]"));
        assert!(plain.contains("Salgo, que la calle ya me espera\n[chorus]\nRefrain en temps réel"));
    }
}

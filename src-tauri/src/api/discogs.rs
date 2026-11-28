//! Client Discogs API pour les éditions physiques (vinyles, maxis club, CD, remixes).
//!
//! Lit `DISCOGS_TOKEN` depuis le fichier `.env` local.
//! Si absent, vide ou égal au placeholder `METTRE_LE_TOKEN_DISCOGS_ICI`, ignore silencieusement
//! en mode cascade `Auto`.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::api::genius;
use crate::models::{MetadataSource, TrackCandidate};
use crate::tagger::rules::{
    RawMetadataInput, normalize_for_match, normalize_track_metadata, split_artist_title,
};

const DISCOGS_SEARCH_URL: &str = "https://api.discogs.com/database/search";
const DISCOGS_USER_AGENT: &str = "TagmasterDJ/0.3.0 +https://github.com/Justinbdna/tagmaster-dj";
const DISCOGS_TIMEOUT_SECS: u64 = 12;
const DISCOGS_RATE_LIMIT_MS: u64 = 1000;
const MAX_DISCOGS_CANDIDATES: usize = 6;

#[derive(Debug, Deserialize)]
struct DiscogsSearchResponse {
    #[serde(default)]
    results: Vec<DiscogsResult>,
}

#[derive(Debug, Deserialize)]
struct DiscogsResult {
    title: Option<String>,
    year: Option<serde_json::Value>,
    #[serde(default)]
    genre: Vec<String>,
    #[serde(default)]
    style: Vec<String>,
    cover_image: Option<String>,
    thumb: Option<String>,
}

fn http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .user_agent(DISCOGS_USER_AGENT)
                .timeout(Duration::from_secs(DISCOGS_TIMEOUT_SECS))
                .build()
                .map_err(|err| format!("Impossible d'initialiser le client HTTP Discogs : {err}"))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

fn rate_limiter() -> &'static Mutex<Option<Instant>> {
    static LIMITER: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LIMITER.get_or_init(|| Mutex::new(None))
}

async fn enforce_rate_limit() {
    let mut last = rate_limiter().lock().await;
    if let Some(prev) = *last {
        let min_gap = Duration::from_millis(DISCOGS_RATE_LIMIT_MS);
        let elapsed = prev.elapsed();
        if elapsed < min_gap {
            tokio::time::sleep(min_gap - elapsed).await;
        }
    }
    *last = Some(Instant::now());
}

fn parse_year_value(val: Option<&serde_json::Value>) -> Option<u32> {
    match val? {
        serde_json::Value::Number(n) => n
            .as_u64()
            .and_then(|y| u32::try_from(y).ok())
            .filter(|y| (1900..=2100).contains(y)),
        serde_json::Value::String(s) => s
            .trim()
            .get(0..4)
            .and_then(|y| y.parse::<u32>().ok())
            .filter(|y| (1900..=2100).contains(y)),
        _ => None,
    }
}

fn clean_discogs_artist_disambiguation(artist: &str) -> String {
    // Discogs ajoute parfois un suffixe numérique d'homonymie : "Madonna (2)" -> "Madonna"
    let trimmed = artist.trim();
    if let Some(open_idx) = trimmed.rfind(" (")
        && trimmed.ends_with(')')
    {
        let inner = &trimmed[open_idx + 2..trimmed.len() - 1];
        if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
            return trimmed[..open_idx].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn valid_discogs_cover(url: Option<&str>) -> Option<String> {
    let u = url?.trim();
    if !u.starts_with("http") || u.contains("spacer.gif") {
        return None;
    }
    Some(u.to_string())
}

fn score_discogs_candidate(query: &str, artist: &str, title: &str) -> f32 {
    let norm_query = normalize_for_match(query);
    let norm_cand = normalize_for_match(&format!("{artist} {title}"));
    if norm_query.is_empty() || norm_cand.is_empty() {
        return 0.5;
    }

    let q_tokens: Vec<&str> = norm_query.split_whitespace().collect();
    let c_tokens: Vec<&str> = norm_cand.split_whitespace().collect();
    let matched = q_tokens
        .iter()
        .filter(|tok| c_tokens.contains(tok))
        .count();
    let recall = matched as f32 / q_tokens.len().max(1) as f32;
    (0.35 + recall * 0.58).clamp(0.35, 0.93)
}

/// Parse la réponse JSON de `/database/search` Discogs en candidats normalisés.
pub fn parse_discogs_json(json: &str, query: &str) -> Result<Vec<TrackCandidate>, String> {
    let payload: DiscogsSearchResponse =
        serde_json::from_str(json).map_err(|err| format!("Réponse JSON Discogs invalide : {err}"))?;

    let mut candidates = Vec::new();
    for item in payload.results {
        let Some(raw_full) = item.title.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };

        let (raw_artist, raw_title) = match split_artist_title(raw_full) {
            Some((a, t)) => (clean_discogs_artist_disambiguation(&a), t),
            None => ("Artiste inconnu".to_string(), raw_full.to_string()),
        };

        let genre = item
            .style
            .first()
            .or_else(|| item.genre.first())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let artwork_url = valid_discogs_cover(item.cover_image.as_deref())
            .or_else(|| valid_discogs_cover(item.thumb.as_deref()));
        let year = parse_year_value(item.year.as_ref());

        let normalized = normalize_track_metadata(RawMetadataInput {
            title: raw_title.clone(),
            artist: raw_artist,
            collection_artist: None,
            album: Some(raw_title),
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            year,
            genre,
            artwork_url,
            is_compilation: false,
            source: MetadataSource::Discogs,
        });

        let score = score_discogs_candidate(query, &normalized.artist, &normalized.title);
        candidates.push(TrackCandidate {
            title: normalized.title,
            artist: normalized.artist,
            album_artist: normalized.album_artist,
            album: normalized.album,
            track_number: normalized.track_number,
            track_total: normalized.track_total,
            disc_number: normalized.disc_number,
            disc_total: normalized.disc_total,
            year: normalized.year,
            genre: normalized.genre,
            artwork_url: normalized.artwork_url,
            is_compilation: normalized.is_compilation,
            storefront: "Discogs".to_string(),
            confidence_score: score,
            source: Some(MetadataSource::Discogs),
            platform: Some("Discogs".to_string()),
            duration_secs: None,
            bpm: None,
        });
    }

    candidates.sort_by(|a, b| b.confidence_score.total_cmp(&a.confidence_score));
    candidates.truncate(MAX_DISCOGS_CANDIDATES);
    Ok(candidates)
}

/// Recherche Discogs avec token explicite (helper testable).
pub async fn search_candidates_with_token(
    token: Option<&str>,
    raw_query: &str,
    error_if_missing: bool,
) -> Result<Vec<TrackCandidate>, String> {
    let clean_token = token
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "METTRE_LE_TOKEN_DISCOGS_ICI");

    let Some(valid_token) = clean_token else {
        if error_if_missing {
            return Err(
                "Token Discogs non configuré : ajoutez DISCOGS_TOKEN dans Paramètres.".to_string(),
            );
        }
        return Ok(Vec::new());
    };

    let cleaned = raw_query.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    let client = http_client()?;
    enforce_rate_limit().await;

    let response = client
        .get(DISCOGS_SEARCH_URL)
        .header("Authorization", format!("Discogs token={valid_token}"))
        .query(&[
            ("q", cleaned.as_str()),
            ("type", "release"),
            ("per_page", "6"),
        ])
        .send()
        .await
        .map_err(|err| format!("Erreur réseau Discogs : {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "API Discogs a répondu avec le statut HTTP {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|err| format!("Impossible de lire la réponse Discogs : {err}"))?;

    parse_discogs_json(&body, &cleaned)
}

/// Recherche Discogs en lisant `DISCOGS_TOKEN` dans `.env`.
pub async fn search_candidates(
    raw_query: &str,
    app: Option<&tauri::AppHandle>,
    error_if_missing: bool,
) -> Result<Vec<TrackCandidate>, String> {
    let token = genius::load_discogs_token(app);
    search_candidates_with_token(token.as_deref(), raw_query, error_if_missing).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ignores_discogs_silently_when_token_is_missing_empty_or_placeholder() {
        assert!(
            search_candidates_with_token(None, "Daft Punk Homework", false)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            search_candidates_with_token(Some("   "), "Daft Punk Homework", false)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            search_candidates_with_token(
                Some("METTRE_LE_TOKEN_DISCOGS_ICI"),
                "Daft Punk Homework",
                false
            )
            .await
            .unwrap()
            .is_empty()
        );
        assert!(
            search_candidates_with_token(None, "Daft Punk Homework", true)
                .await
                .is_err()
        );
    }

    #[test]
    fn parses_discogs_release_and_cleans_artist_disambiguation() {
        let sample = r#"{
            "results": [
                {
                    "title": "Daft Punk (2) - Da Funk (12\" Club Mix)",
                    "year": "1995",
                    "genre": ["Electronic"],
                    "style": ["French House"],
                    "cover_image": "https://i.discogs.com/cover_hd.jpg"
                }
            ]
        }"#;

        let results = parse_discogs_json(sample, "Daft Punk Da Funk").expect("valid discogs json");
        assert_eq!(results.len(), 1);
        let cand = &results[0];
        assert_eq!(cand.artist, "Daft Punk");
        assert_eq!(cand.title, "Da Funk (12\" Club Mix)");
        assert_eq!(cand.year, Some(1995));
        assert_eq!(cand.genre.as_deref(), Some("French House"));
        assert_eq!(cand.artwork_url.as_deref(), Some("https://i.discogs.com/cover_hd.jpg"));
        assert_eq!(cand.source, Some(MetadataSource::Discogs));
    }
}

//! Client API publique Deezer (sans clé API).
//!
//! Utilise `https://api.deezer.com/search` pour la recherche de pistes et
//! `/album/{id}` pour enrichir l'année de sortie, le genre et `album_artist`.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::models::{MetadataSource, TrackCandidate};
use crate::tagger::rules::{RawMetadataInput, normalize_for_match, normalize_track_metadata};

const DEEZER_SEARCH_URL: &str = "https://api.deezer.com/search";
const DEEZER_ALBUM_URL: &str = "https://api.deezer.com/album";
const DEEZER_TIMEOUT_SECS: u64 = 12;
const DEEZER_MIN_GAP_MS: u64 = 350;
const MAX_DEEZER_CANDIDATES: usize = 6;

#[derive(Debug, Deserialize)]
struct DeezerSearchResponse {
    #[serde(default)]
    data: Vec<DeezerTrack>,
}

#[derive(Debug, Deserialize)]
struct DeezerTrack {
    title: Option<String>,
    duration: Option<u64>,
    track_position: Option<u32>,
    disk_number: Option<u32>,
    artist: Option<DeezerArtist>,
    album: Option<DeezerAlbum>,
}

#[derive(Debug, Deserialize)]
struct DeezerArtist {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeezerAlbum {
    id: Option<u64>,
    title: Option<String>,
    cover_xl: Option<String>,
    cover_big: Option<String>,
    release_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeezerAlbumDetail {
    release_date: Option<String>,
    nb_tracks: Option<u32>,
    artist: Option<DeezerArtist>,
    genres: Option<DeezerGenresEnvelope>,
}

#[derive(Debug, Deserialize)]
struct DeezerGenresEnvelope {
    #[serde(default)]
    data: Vec<DeezerGenre>,
}

#[derive(Debug, Deserialize)]
struct DeezerGenre {
    name: Option<String>,
}

fn http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .user_agent("TagmasterDJ/0.2.0 (Open Source Local Music Tagger)")
                .timeout(Duration::from_secs(DEEZER_TIMEOUT_SECS))
                .build()
                .map_err(|err| format!("Impossible d'initialiser le client HTTP Deezer : {err}"))
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
        let min_gap = Duration::from_millis(DEEZER_MIN_GAP_MS);
        let elapsed = prev.elapsed();
        if elapsed < min_gap {
            tokio::time::sleep(min_gap - elapsed).await;
        }
    }
    *last = Some(Instant::now());
}

fn extract_year(date: Option<&str>) -> Option<u32> {
    date?
        .get(0..4)?
        .parse::<u32>()
        .ok()
        .filter(|y| (1900..=2100).contains(y))
}

fn score_candidate(query: &str, artist: &str, title: &str) -> f32 {
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

    if matched == 0 {
        return 0.35;
    }

    let recall = matched as f32 / q_tokens.len().max(1) as f32;
    let dice = (2.0 * matched as f32) / (q_tokens.len() + c_tokens.len()).max(1) as f32;
    (recall * 0.7 + dice * 0.3).clamp(0.4, 0.96)
}

/// Parse la réponse JSON de `/search` Deezer et retourne `(album_id, TrackCandidate)` triés par pertinence.
pub fn parse_deezer_search_json(
    json: &str,
    query: &str,
) -> Result<Vec<(Option<u64>, TrackCandidate)>, String> {
    let envelope: DeezerSearchResponse =
        serde_json::from_str(json).map_err(|err| format!("Réponse JSON Deezer invalide : {err}"))?;

    let mut parsed = Vec::new();
    for track in envelope.data {
        let Some(raw_title) = track.title.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some(raw_artist) = track
            .artist
            .as_ref()
            .and_then(|a| a.name.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };

        let album_id = track.album.as_ref().and_then(|a| a.id);
        let raw_album = track
            .album
            .as_ref()
            .and_then(|a| a.title.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let artwork_url = track.album.as_ref().and_then(|a| {
            a.cover_xl
                .as_deref()
                .or(a.cover_big.as_deref())
                .map(str::trim)
                .filter(|u| u.starts_with("http"))
                .map(str::to_string)
        });
        let year = track
            .album
            .as_ref()
            .and_then(|a| extract_year(a.release_date.as_deref()));

        let normalized = normalize_track_metadata(RawMetadataInput {
            title: raw_title.to_string(),
            artist: raw_artist.to_string(),
            collection_artist: None,
            album: raw_album,
            track_number: track.track_position,
            track_total: None,
            disc_number: track.disk_number,
            disc_total: None,
            year,
            genre: None,
            artwork_url,
            is_compilation: false,
            source: MetadataSource::Deezer,
        });

        let score = score_candidate(query, &normalized.artist, &normalized.title);
        parsed.push((
            album_id,
            TrackCandidate {
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
                storefront: "Deezer".to_string(),
                confidence_score: score,
                source: Some(MetadataSource::Deezer),
                platform: Some("Deezer".to_string()),
                duration_secs: track.duration.filter(|&d| d > 0),
            },
        ));
    }

    parsed.sort_by(|a, b| b.1.confidence_score.total_cmp(&a.1.confidence_score));
    parsed.truncate(MAX_DEEZER_CANDIDATES);
    Ok(parsed)
}

async fn fetch_album_detail(album_id: u64) -> Option<DeezerAlbumDetail> {
    let client = http_client().ok()?;
    enforce_rate_limit().await;
    let url = format!("{DEEZER_ALBUM_URL}/{album_id}");
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<DeezerAlbumDetail>().await.ok()
}

/// Recherche publique Deezer (`https://api.deezer.com/search`) avec enrichissement album sur les meilleurs résultats.
pub async fn search_candidates(raw_query: &str) -> Result<Vec<TrackCandidate>, String> {
    let cleaned = raw_query.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    let client = http_client()?;
    enforce_rate_limit().await;

    let response = client
        .get(DEEZER_SEARCH_URL)
        .query(&[("q", cleaned.as_str()), ("limit", "8")])
        .send()
        .await
        .map_err(|err| format!("Erreur réseau Deezer : {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "API Deezer a répondu avec le statut HTTP {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|err| format!("Impossible de lire la réponse Deezer : {err}"))?;

    let mut parsed = parse_deezer_search_json(&body, &cleaned)?;

    // Enrichit les 2 meilleurs candidats via /album/{id} (année, genre, album_artist, track_total)
    for (album_id_opt, cand) in parsed.iter_mut().take(2) {
        if let Some(album_id) = *album_id_opt
            && (cand.year.is_none() || cand.genre.is_none())
            && let Some(detail) = fetch_album_detail(album_id).await
        {
            if cand.year.is_none() {
                cand.year = extract_year(detail.release_date.as_deref());
            }
            if cand.track_total.is_none() {
                cand.track_total = detail.nb_tracks;
            }
            if let Some(alb_artist) = detail
                .artist
                .and_then(|a| a.name)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                cand.album_artist = alb_artist;
            }
            if cand.genre.is_none() {
                cand.genre = detail
                    .genres
                    .and_then(|g| g.data.into_iter().next())
                    .and_then(|g| g.name)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
            }
        }
    }

    Ok(parsed.into_iter().map(|(_, cand)| cand).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_deezer_search_json_with_cover_xl_and_duration() {
        let sample = r#"{
            "data": [
                {
                    "title": "Get Lucky (feat. Pharrell Williams)",
                    "duration": 248,
                    "track_position": 8,
                    "disk_number": 1,
                    "artist": { "name": "Daft Punk" },
                    "album": {
                        "id": 6575789,
                        "title": "Random Access Memories",
                        "cover_xl": "https://e-cdns-images.dzcdn.net/images/cover/abc/1000x1000-000000-80-0-0.jpg",
                        "release_date": "2013-05-17"
                    }
                }
            ]
        }"#;

        let results =
            parse_deezer_search_json(sample, "Daft Punk Get Lucky").expect("valid deezer json");
        assert_eq!(results.len(), 1);
        let (album_id, cand) = &results[0];
        assert_eq!(*album_id, Some(6575789));
        assert_eq!(cand.artist, "Daft Punk");
        assert_eq!(cand.title, "Get Lucky (feat. Pharrell Williams)");
        assert_eq!(cand.album, "Random Access Memories");
        assert_eq!(cand.year, Some(2013));
        assert_eq!(cand.duration_secs, Some(248));
        assert_eq!(cand.source, Some(MetadataSource::Deezer));
        assert_eq!(cand.platform.as_deref(), Some("Deezer"));
        assert!(cand.artwork_url.as_deref().unwrap().contains("1000x1000"));
    }
}

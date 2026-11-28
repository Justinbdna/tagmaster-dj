//! Client API MusicBrainz (sans clé API) + Cover Art Archive.
//!
//! Respecte strictement le rate-limit MusicBrainz (1 requête/seconde) et envoie
//! le header `User-Agent: TagmasterDJ/0.3.0`.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::models::{MetadataSource, TrackCandidate};
use crate::tagger::rules::{RawMetadataInput, normalize_for_match, normalize_track_metadata};

const MUSICBRAINZ_RECORDING_URL: &str = "https://musicbrainz.org/ws/2/recording/";
const COVER_ART_ARCHIVE_BASE: &str = "https://coverartarchive.org/release";
const MB_USER_AGENT: &str = "TagmasterDJ/0.3.0 (https://github.com/Justinbdna/tagmaster-dj)";
const MB_TIMEOUT_SECS: u64 = 12;
const MB_RATE_LIMIT_MS: u64 = 1050;
const MAX_MB_CANDIDATES: usize = 6;

#[derive(Debug, Deserialize)]
struct MbSearchResponse {
    #[serde(default)]
    recordings: Vec<MbRecording>,
}

#[derive(Debug, Deserialize)]
struct MbRecording {
    title: Option<String>,
    length: Option<u64>,
    score: Option<u32>,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<MbArtistCredit>,
    #[serde(default)]
    releases: Vec<MbRelease>,
    #[serde(default)]
    tags: Vec<MbTag>,
}

#[derive(Debug, Deserialize)]
struct MbArtistCredit {
    name: Option<String>,
    joinphrase: Option<String>,
    artist: Option<MbArtist>,
}

#[derive(Debug, Deserialize)]
struct MbArtist {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MbRelease {
    id: Option<String>,
    title: Option<String>,
    date: Option<String>,
    #[serde(rename = "track-count")]
    track_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct MbTag {
    name: Option<String>,
    count: Option<i32>,
}

fn http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .user_agent(MB_USER_AGENT)
                .timeout(Duration::from_secs(MB_TIMEOUT_SECS))
                .build()
                .map_err(|err| {
                    format!("Impossible d'initialiser le client HTTP MusicBrainz : {err}")
                })
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
        let min_gap = Duration::from_millis(MB_RATE_LIMIT_MS);
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

fn join_artist_credits(credits: &[MbArtistCredit]) -> Option<String> {
    let mut out = String::new();
    for credit in credits {
        let name = credit
            .name
            .as_deref()
            .or_else(|| credit.artist.as_ref().and_then(|a| a.name.as_deref()))
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(n) = name {
            out.push_str(n);
            if let Some(join) = credit.joinphrase.as_deref() {
                out.push_str(join);
            }
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn best_tag_genre(tags: &[MbTag]) -> Option<String> {
    tags.iter()
        .filter_map(|t| {
            let name = t.name.as_deref()?.trim();
            if name.is_empty() {
                return None;
            }
            Some((t.count.unwrap_or(0), name))
        })
        .max_by_key(|(count, _)| *count)
        .map(|(_, name)| name.to_string())
}

fn score_mb_candidate(query: &str, artist: &str, title: &str, api_score: Option<u32>) -> f32 {
    let api_norm = api_score.map(|s| (s as f32 / 100.0).clamp(0.0, 1.0));
    let norm_query = normalize_for_match(query);
    let norm_cand = normalize_for_match(&format!("{artist} {title}"));
    if norm_query.is_empty() || norm_cand.is_empty() {
        return api_norm.unwrap_or(0.5);
    }

    let q_tokens: Vec<&str> = norm_query.split_whitespace().collect();
    let c_tokens: Vec<&str> = norm_cand.split_whitespace().collect();
    let matched = q_tokens
        .iter()
        .filter(|tok| c_tokens.contains(tok))
        .count();
    let recall = matched as f32 / q_tokens.len().max(1) as f32;
    let combined = match api_norm {
        Some(a) => a * 0.55 + recall * 0.45,
        None => recall,
    };
    combined.clamp(0.35, 0.95)
}

/// Parse la réponse JSON de `/ws/2/recording/` MusicBrainz en candidats normalisés.
pub fn parse_musicbrainz_json(json: &str, query: &str) -> Result<Vec<TrackCandidate>, String> {
    let payload: MbSearchResponse = serde_json::from_str(json)
        .map_err(|err| format!("Réponse JSON MusicBrainz invalide : {err}"))?;

    let mut candidates = Vec::new();
    for rec in payload.recordings {
        let Some(raw_title) = rec.title.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some(raw_artist) = join_artist_credits(&rec.artist_credit) else {
            continue;
        };

        let best_release = rec
            .releases
            .iter()
            .find(|r| r.date.is_some() && r.id.is_some())
            .or_else(|| rec.releases.first());

        let raw_album = best_release
            .and_then(|r| r.title.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let year = best_release.and_then(|r| extract_year(r.date.as_deref()));
        let track_total = best_release.and_then(|r| r.track_count);
        let artwork_url = best_release
            .and_then(|r| r.id.as_deref())
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| format!("{COVER_ART_ARCHIVE_BASE}/{id}/front-500"));
        let genre = best_tag_genre(&rec.tags);
        let duration_secs = rec.length.map(|ms| ms / 1000).filter(|&s| s > 0);

        let normalized = normalize_track_metadata(RawMetadataInput {
            title: raw_title.to_string(),
            artist: raw_artist,
            collection_artist: None,
            album: raw_album,
            track_number: None,
            track_total,
            disc_number: None,
            disc_total: None,
            year,
            genre,
            artwork_url,
            is_compilation: false,
            source: MetadataSource::MusicBrainz,
        });

        let score = score_mb_candidate(query, &normalized.artist, &normalized.title, rec.score);
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
            storefront: "MusicBrainz".to_string(),
            confidence_score: score,
            source: Some(MetadataSource::MusicBrainz),
            platform: Some("MusicBrainz".to_string()),
            duration_secs,
            bpm: None,
        });
    }

    candidates.sort_by(|a, b| b.confidence_score.total_cmp(&a.confidence_score));
    candidates.truncate(MAX_MB_CANDIDATES);
    Ok(candidates)
}

/// Recherche MusicBrainz avec délai anti-rate-limit (1s) et pochettes Cover Art Archive.
pub async fn search_candidates(raw_query: &str) -> Result<Vec<TrackCandidate>, String> {
    let cleaned = raw_query.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    let client = http_client()?;
    enforce_rate_limit().await;

    let response = client
        .get(MUSICBRAINZ_RECORDING_URL)
        .query(&[("query", cleaned.as_str()), ("fmt", "json"), ("limit", "6")])
        .send()
        .await
        .map_err(|err| format!("Erreur réseau MusicBrainz : {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "API MusicBrainz a répondu avec le statut HTTP {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|err| format!("Impossible de lire la réponse MusicBrainz : {err}"))?;

    parse_musicbrainz_json(&body, &cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_musicbrainz_recording_and_builds_cover_art_archive_url() {
        let sample = r#"{
            "recordings": [
                {
                    "title": "Around the World",
                    "length": 429000,
                    "score": 98,
                    "artist-credit": [
                        { "name": "Daft Punk", "artist": { "name": "Daft Punk" } }
                    ],
                    "releases": [
                        {
                            "id": "12345678-aaaa-bbbb-cccc-1234567890ab",
                            "title": "Homework",
                            "date": "1997-01-20",
                            "track-count": 16
                        }
                    ],
                    "tags": [
                        { "name": "french house", "count": 12 },
                        { "name": "electronic", "count": 5 }
                    ]
                }
            ]
        }"#;

        let results = parse_musicbrainz_json(sample, "Daft Punk Around the World")
            .expect("valid musicbrainz json");
        assert_eq!(results.len(), 1);
        let cand = &results[0];
        assert_eq!(cand.artist, "Daft Punk");
        assert_eq!(cand.title, "Around the World");
        assert_eq!(cand.album, "Homework");
        assert_eq!(cand.year, Some(1997));
        assert_eq!(cand.duration_secs, Some(429));
        assert_eq!(cand.source, Some(MetadataSource::MusicBrainz));
        assert_eq!(
            cand.artwork_url.as_deref(),
            Some("https://coverartarchive.org/release/12345678-aaaa-bbbb-cccc-1234567890ab/front-500")
        );
    }
}

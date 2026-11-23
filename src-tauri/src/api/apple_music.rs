//! Client Apple Music / iTunes Search API (SPECIFICATIONS.md §4.2).
//!
//! - Storefront anglophone forcé : `US` par défaut, fallback automatique `GB` (UK)
//!   pour uniformiser les genres (Pop, R&B/Soul, Hip-Hop/Rap, Electronic).
//! - Anti-ban intégré : pause automatique aléatoire entre 1 000 ms et 2 000 ms
//!   entre chaque requête HTTP sortante.
//! - Upgrade automatique des pochettes vers 1000x1000px HD.
//! - Respect strict des mots-clés intouchables (§4.3) : ne jamais écraser un Live,
//!   Remix, Acoustic, Sped Up, Chopped, Instrumental ou Mashup par une version studio.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rand::RngExt;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::models::{LookupStatus, MetadataLookupResult, MetadataSource};
use crate::tagger::rules::{
    RawMetadataInput, build_search_query, detect_protected_keyword, normalize_track_metadata,
};

pub const ITUNES_SEARCH_URL: &str = "https://itunes.apple.com/search";
pub const DEFAULT_STOREFRONT: &str = "US";
pub const FALLBACK_STOREFRONT: &str = "GB";
pub const RATE_LIMIT_MIN_MS: u64 = 1000;
pub const RATE_LIMIT_MAX_MS: u64 = 2000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesSearchResponse {
    #[serde(default)]
    results: Vec<ItunesTrack>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesTrack {
    wrapper_type: Option<String>,
    kind: Option<String>,
    track_name: Option<String>,
    artist_name: Option<String>,
    collection_artist_name: Option<String>,
    collection_name: Option<String>,
    track_number: Option<u32>,
    track_count: Option<u32>,
    disc_number: Option<u32>,
    disc_count: Option<u32>,
    release_date: Option<String>,
    primary_genre_name: Option<String>,
    artwork_url100: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadedArtwork {
    pub bytes: Vec<u8>,
    pub mime_type: String,
}

fn http_client() -> Result<&'static Client, String> {
    static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .user_agent("TagmasterDJ/0.1.0 (Open Source Local Music Tagger)")
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|err| format!("Impossible d'initialiser le client HTTP : {err}"))
        })
        .as_ref()
        .map_err(|e| e.clone())
}

fn rate_limiter() -> &'static Mutex<Option<Instant>> {
    static LIMITER: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LIMITER.get_or_init(|| Mutex::new(None))
}

/// Garantit un délai aléatoire de 1 à 2 secondes entre deux requêtes Apple Music API.
async fn enforce_rate_limit() {
    let mut last_request = rate_limiter().lock().await;
    if let Some(prev) = *last_request {
        let min_gap_ms = rand::rng().random_range(RATE_LIMIT_MIN_MS..=RATE_LIMIT_MAX_MS);
        let min_gap = Duration::from_millis(min_gap_ms);
        let elapsed = prev.elapsed();
        if elapsed < min_gap {
            tokio::time::sleep(min_gap - elapsed).await;
        }
    }
    *last_request = Some(Instant::now());
}

/// Transforme une URL de miniature iTunes (`100x100bb.jpg`) en pochette HD (`1000x1000bb.jpg`).
pub fn upgrade_artwork_url_hd(url: &str) -> String {
    if url.contains("100x100bb") {
        url.replace("100x100bb", "1000x1000bb")
    } else if url.contains("100x100") {
        url.replace("100x100", "1000x1000")
    } else {
        url.to_string()
    }
}

fn extract_year(release_date: Option<&str>) -> Option<u32> {
    release_date?
        .get(0..4)?
        .parse::<u32>()
        .ok()
        .filter(|y| (1900..=2100).contains(y))
}

fn normalize_for_match(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn token_overlap_score(expected: &str, candidate: &str) -> f32 {
    let exp = normalize_for_match(expected);
    let cand = normalize_for_match(candidate);
    if exp.is_empty() || cand.is_empty() {
        return 0.0;
    }
    if exp == cand {
        return 1.0;
    }
    if cand.contains(&exp) || exp.contains(&cand) {
        return 0.85;
    }

    let exp_tokens: Vec<&str> = exp.split_whitespace().collect();
    let cand_tokens: Vec<&str> = cand.split_whitespace().collect();
    let matched = exp_tokens
        .iter()
        .filter(|tok| cand_tokens.contains(tok))
        .count();

    matched as f32 / exp_tokens.len().max(1) as f32
}

fn score_candidate(
    candidate: &ItunesTrack,
    artist_hint: Option<&str>,
    title_hint: &str,
) -> f32 {
    let cand_title = candidate.track_name.as_deref().unwrap_or("");
    let cand_artist = candidate.artist_name.as_deref().unwrap_or("");
    let cand_album = candidate.collection_name.as_deref().unwrap_or("");

    // Rejeter les pistes karaoke / tribute / versions alternatives non demandées
    let lower_title = cand_title.to_lowercase();
    let lower_album = cand_album.to_lowercase();
    if lower_title.contains("karaoke")
        || lower_album.contains("karaoke")
        || lower_album.contains("tribute to")
        || detect_protected_keyword(cand_title).is_some()
    {
        return -1.0;
    }

    let title_score = token_overlap_score(title_hint, cand_title);
    let artist_score = match artist_hint {
        Some(artist) => token_overlap_score(artist, cand_artist),
        None => 0.5,
    };

    let mut total = title_score * 0.65 + artist_score * 0.35;

    // Léger bonus si c'est un vrai album complet plutôt qu'une compilation générique
    if candidate
        .collection_artist_name
        .as_deref()
        .map(|s| !s.eq_ignore_ascii_case("Various Artists"))
        .unwrap_or(true)
    {
        total += 0.03;
    }

    total
}

async fn query_storefront(term: &str, storefront: &str) -> Result<Vec<ItunesTrack>, String> {
    enforce_rate_limit().await;

    let client = http_client()?;
    let response = client
        .get(ITUNES_SEARCH_URL)
        .query(&[
            ("term", term),
            ("country", storefront),
            ("media", "music"),
            ("entity", "song"),
            ("limit", "10"),
        ])
        .send()
        .await
        .map_err(|err| format!("Erreur réseau Apple Music ({storefront}) : {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "API Apple Music ({storefront}) a répondu avec le statut HTTP {}",
            response.status()
        ));
    }

    let payload: ItunesSearchResponse = response
        .json()
        .await
        .map_err(|err| format!("Réponse JSON Apple Music invalide : {err}"))?;

    Ok(payload
        .results
        .into_iter()
        .filter(|r| {
            r.wrapper_type.as_deref() == Some("track")
                && r.kind.as_deref().unwrap_or("song") == "song"
                && r.track_name.is_some()
                && r.artist_name.is_some()
        })
        .collect())
}

/// Recherche un morceau sur Apple Music (`US`, puis fallback `GB`) en respectant
/// les mots-clés intouchables (§4.3) et le rate limiting anti-ban (§4.2).
pub async fn search_track(
    file_name: &str,
    existing_artist: Option<&str>,
    existing_title: Option<&str>,
) -> Result<MetadataLookupResult, String> {
    // 1. Garde-fou absolu : mots-clés intouchables dans le nom de fichier ou le titre existant
    if let Some(kw) = detect_protected_keyword(file_name)
        .or_else(|| existing_title.and_then(detect_protected_keyword))
    {
        return Ok(MetadataLookupResult {
            status: LookupStatus::ProtectedVersion,
            query_used: file_name.to_string(),
            storefront: None,
            metadata: None,
            detected_keyword: Some(kw.to_string()),
            reason: Some(format!(
                "Mot-clé protégé « {kw} » détecté : Apple Music bloqué pour éviter d'écraser par une version studio (yt-dlp / révision manuelle)."
            )),
        });
    }

    let parsed = build_search_query(file_name, existing_artist, existing_title);
    if parsed.search_term.trim().is_empty() {
        return Ok(MetadataLookupResult {
            status: LookupStatus::NotFound,
            query_used: String::new(),
            storefront: None,
            metadata: None,
            detected_keyword: None,
            reason: Some("Impossible d'extraire un titre exploitable depuis le fichier.".to_string()),
        });
    }

    // 2. Storefront forcé US, puis fallback GB (UK)
    for storefront in [DEFAULT_STOREFRONT, FALLBACK_STOREFRONT] {
        let candidates = query_storefront(&parsed.search_term, storefront).await?;
        let best = candidates
            .into_iter()
            .map(|c| {
                let score = score_candidate(&c, parsed.artist_hint.as_deref(), &parsed.title_hint);
                (score, c)
            })
            .filter(|(score, _)| *score >= 0.55)
            .max_by(|a, b| a.0.total_cmp(&b.0));

        if let Some((_score, track)) = best {
            let artwork_hd = track
                .artwork_url100
                .as_deref()
                .map(upgrade_artwork_url_hd);

            let normalized = normalize_track_metadata(RawMetadataInput {
                title: track.track_name.unwrap_or_default(),
                artist: track.artist_name.unwrap_or_default(),
                collection_artist: track.collection_artist_name,
                album: track.collection_name,
                track_number: track.track_number,
                track_total: track.track_count,
                disc_number: track.disc_number,
                disc_total: track.disc_count,
                year: extract_year(track.release_date.as_deref()),
                genre: track.primary_genre_name,
                artwork_url: artwork_hd,
                is_compilation: false,
                source: MetadataSource::AppleMusic,
            });

            return Ok(MetadataLookupResult {
                status: LookupStatus::Matched,
                query_used: parsed.search_term,
                storefront: Some(storefront.to_string()),
                metadata: Some(normalized),
                detected_keyword: None,
                reason: None,
            });
        }
    }

    Ok(MetadataLookupResult {
        status: LookupStatus::NotFound,
        query_used: parsed.search_term,
        storefront: None,
        metadata: None,
        detected_keyword: None,
        reason: Some(
            "Aucune correspondance fiable trouvée sur Apple Music (US / UK) — à envoyer en file de révision."
                .to_string(),
        ),
    })
}

/// Télécharge la pochette HD (JPEG / PNG) prête à être intégrée dans les tags audio.
pub async fn download_artwork(url: &str) -> Result<DownloadedArtwork, String> {
    let client = http_client()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| format!("Erreur lors du téléchargement de la pochette : {err}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Échec du téléchargement de la pochette (HTTP {})",
            response.status()
        ));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Impossible de lire les octets de la pochette : {err}"))?
        .to_vec();

    if bytes.is_empty() {
        return Err("Pochette téléchargée vide.".to_string());
    }

    let mime_type = if content_type.contains("png") || bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47])
    {
        "image/png".to_string()
    } else {
        "image/jpeg".to_string()
    };

    Ok(DownloadedArtwork { bytes, mime_type })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_itunes_artwork_to_1000px_hd() {
        let low = "https://is1-ssl.mzstatic.com/image/thumb/Music126/v4/aa/bb/cc/source/100x100bb.jpg";
        let hd = upgrade_artwork_url_hd(low);
        assert!(hd.ends_with("1000x1000bb.jpg"));
    }

    #[test]
    fn rejects_studio_lookup_for_protected_versions() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();

        let result = rt
            .block_on(search_track(
                "01 - The Sims Theme (Junkie XL Remix).mp3",
                None,
                None,
            ))
            .expect("should return ProtectedVersion without network call");

        assert_eq!(result.status, LookupStatus::ProtectedVersion);
        assert_eq!(result.detected_keyword.as_deref(), Some("remix"));
        assert!(result.metadata.is_none());
    }

    #[test]
    fn scores_exact_match_above_karaoke_or_live() {
        let studio = ItunesTrack {
            wrapper_type: Some("track".into()),
            kind: Some("song".into()),
            track_name: Some("Get Lucky (feat. Pharrell Williams & Nile Rodgers)".into()),
            artist_name: Some("Daft Punk".into()),
            collection_artist_name: Some("Daft Punk".into()),
            collection_name: Some("Random Access Memories".into()),
            track_number: Some(8),
            track_count: Some(13),
            disc_number: Some(1),
            disc_count: Some(1),
            release_date: Some("2013-05-17T07:00:00Z".into()),
            primary_genre_name: Some("Electronic".into()),
            artwork_url100: None,
        };
        let karaoke = ItunesTrack {
            track_name: Some("Get Lucky (Karaoke Version)".into()),
            ..studio.clone()
        };

        assert!(score_candidate(&studio, Some("Daft Punk"), "Get Lucky") > 0.8);
        assert!(score_candidate(&karaoke, Some("Daft Punk"), "Get Lucky") < 0.0);
    }
}

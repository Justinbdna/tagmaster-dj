//! Client Apple Music / iTunes Search API (SPECIFICATIONS.md §4.2).
//!
//! - Cascade de storefronts : `US` par défaut, fallback `GB` (UK), puis fallback `FR`
//!   (avec normalisation des genres français vers l'anglais pour garder la bibliothèque unifiée).
//! - Fallback lookup catalogue artiste (`musicArtist` + `/lookup`) pour les titres courts
//!   ou francophones tronqués par `/search` (ex: `Shay - T mimi`, `PNL - Au DD`).
//! - Anti-ban intégré : pause automatique aléatoire entre 1 000 ms et 2 000 ms
//!   entre chaque requête HTTP sortante.
//! - Upgrade automatique des pochettes vers 1000x1000px HD.
//! - Respect strict des mots-clés intouchables (§4.3) : ne jamais écraser un Live,
//!   Remix, Acoustic, Sped Up, Chopped, Instrumental ou Mashup par une version studio.

use std::future::Future;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rand::RngExt;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::models::{ExistingTrackTags, LookupStatus, MetadataLookupResult, MetadataSource};
use crate::tagger::rules::{
    ParsedTrackQuery, RawMetadataInput, build_search_query, detect_protected_keyword,
    extract_primary_artist, normalize_for_match, normalize_track_metadata, strip_feat_from_title,
};

pub const ITUNES_SEARCH_URL: &str = "https://itunes.apple.com/search";
pub const ITUNES_LOOKUP_URL: &str = "https://itunes.apple.com/lookup";
pub const DEFAULT_STOREFRONT: &str = "US";
pub const FALLBACK_STOREFRONT: &str = "GB";
pub const FRENCH_FALLBACK_STOREFRONT: &str = "FR";
pub const STOREFRONT_CASCADE: &[&str] = &[
    DEFAULT_STOREFRONT,
    FALLBACK_STOREFRONT,
    FRENCH_FALLBACK_STOREFRONT,
];
pub const RATE_LIMIT_MIN_MS: u64 = 1000;
pub const RATE_LIMIT_MAX_MS: u64 = 2000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesSearchResponse {
    #[serde(default)]
    results: Vec<ItunesTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesArtistSearchResponse {
    #[serde(default)]
    results: Vec<ItunesArtist>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesArtist {
    wrapper_type: Option<String>,
    artist_id: Option<u64>,
    artist_name: Option<String>,
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

fn is_contiguous_subslice(haystack: &[&str], needle: &[&str]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
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

    let exp_tokens: Vec<&str> = exp.split_whitespace().collect();
    let cand_tokens: Vec<&str> = cand.split_whitespace().collect();

    if is_contiguous_subslice(&cand_tokens, &exp_tokens)
        && (exp_tokens.len() >= 2
            || (exp_tokens.len() == 1
                && cand_tokens.len() <= 2
                && cand_tokens[0] == exp_tokens[0]))
    {
        return 0.88;
    }

    let matched = exp_tokens
        .iter()
        .filter(|tok| cand_tokens.contains(tok))
        .count();
    if matched == 0 {
        return 0.0;
    }

    let recall = matched as f32 / exp_tokens.len().max(1) as f32;
    let dice = (2.0 * matched as f32) / (exp_tokens.len() + cand_tokens.len()).max(1) as f32;
    recall.min(dice)
}

fn is_distinctive_title(title: &str) -> bool {
    let norm = normalize_for_match(title);
    let char_len = norm.chars().filter(|c| !c.is_whitespace()).count();
    let word_count = norm.split_whitespace().count();
    char_len >= 8 || word_count >= 3
}

fn score_candidate(
    candidate: &ItunesTrack,
    artist_hint: Option<&str>,
    title_hint: &str,
    album_hint: Option<&str>,
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

    let stripped_cand_title = strip_feat_from_title(cand_title);
    let title_score = token_overlap_score(title_hint, cand_title)
        .max(token_overlap_score(title_hint, &stripped_cand_title));

    let non_compilation_bonus = if candidate
        .collection_artist_name
        .as_deref()
        .map(|s| !s.eq_ignore_ascii_case("Various Artists"))
        .unwrap_or(true)
    {
        0.03
    } else {
        0.0
    };

    match (artist_hint, album_hint) {
        (Some(artist), _) => {
            let primary_cand_artist = extract_primary_artist(cand_artist);
            let mut artist_score = token_overlap_score(artist, cand_artist)
                .max(token_overlap_score(artist, &primary_cand_artist));

            if let Some(col_artist) = candidate.collection_artist_name.as_deref() {
                let primary_col = extract_primary_artist(col_artist);
                artist_score = artist_score
                    .max(token_overlap_score(artist, col_artist))
                    .max(token_overlap_score(artist, &primary_col));
            }

            if title_score < 0.60 || artist_score < 0.68 {
                return -1.0;
            }

            let mut total = title_score * 0.62 + artist_score * 0.35 + non_compilation_bonus;
            if let Some(album) = album_hint
                && token_overlap_score(album, cand_album) >= 0.70
            {
                total += 0.08;
            }
            total
        }
        (None, Some(album)) => {
            let album_score = token_overlap_score(album, cand_album);
            if title_score < 0.85 || album_score < 0.75 {
                return -1.0;
            }
            title_score * 0.60 + album_score * 0.38 + non_compilation_bonus
        }
        (None, None) => {
            if !is_distinctive_title(title_hint) || title_score < 0.99 {
                return -1.0;
            }
            title_score * 0.85 + non_compilation_bonus
        }
    }
}

fn pick_best_candidate(
    candidates: Vec<ItunesTrack>,
    parsed: &ParsedTrackQuery,
) -> Option<ItunesTrack> {
    let min_score = if parsed.artist_hint.is_none() && parsed.album_hint.is_none() {
        0.80
    } else {
        0.68
    };

    let scored: Vec<(f32, ItunesTrack)> = candidates
        .into_iter()
        .map(|c| {
            let score = score_candidate(
                &c,
                parsed.artist_hint.as_deref(),
                &parsed.title_hint,
                parsed.album_hint.as_deref(),
            );
            (score, c)
        })
        .filter(|(score, _)| *score >= min_score)
        .collect();

    if parsed.artist_hint.is_none() && parsed.album_hint.is_none() {
        let mut distinct_artists: Vec<String> = Vec::new();
        for (_, track) in &scored {
            if let Some(artist) = track.artist_name.as_deref() {
                let norm = normalize_for_match(&extract_primary_artist(artist));
                if !norm.is_empty() && !distinct_artists.contains(&norm) {
                    distinct_artists.push(norm);
                }
            }
        }
        if distinct_artists.len() > 1 {
            return None;
        }
    }

    scored
        .into_iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, track)| track)
}

fn filter_song_tracks(results: Vec<ItunesTrack>) -> Vec<ItunesTrack> {
    results
        .into_iter()
        .filter(|r| {
            r.wrapper_type.as_deref() == Some("track")
                && r.kind.as_deref().unwrap_or("song") == "song"
                && r.track_name.is_some()
                && r.artist_name.is_some()
        })
        .collect()
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

    Ok(filter_song_tracks(payload.results))
}

async fn query_artist_catalog(artist_hint: &str, storefront: &str) -> Result<Vec<ItunesTrack>, String> {
    enforce_rate_limit().await;

    let client = http_client()?;
    let artist_response = client
        .get(ITUNES_SEARCH_URL)
        .query(&[
            ("term", artist_hint),
            ("country", storefront),
            ("media", "music"),
            ("entity", "musicArtist"),
            ("limit", "5"),
        ])
        .send()
        .await
        .map_err(|err| format!("Erreur réseau Apple Music ({storefront}) : {err}"))?;

    if !artist_response.status().is_success() {
        return Err(format!(
            "API Apple Music ({storefront}) a répondu avec le statut HTTP {}",
            artist_response.status()
        ));
    }

    let artist_payload: ItunesArtistSearchResponse = artist_response
        .json()
        .await
        .map_err(|err| format!("Réponse JSON artiste Apple Music invalide : {err}"))?;

    let mut artist_ids: Vec<u64> = Vec::new();
    for entry in artist_payload.results {
        if entry.wrapper_type.as_deref() != Some("artist") {
            continue;
        }
        let Some(id) = entry.artist_id else {
            continue;
        };
        let Some(name) = entry.artist_name.as_deref() else {
            continue;
        };
        if token_overlap_score(artist_hint, name) >= 0.75 && !artist_ids.contains(&id) {
            artist_ids.push(id);
            if artist_ids.len() == 2 {
                break;
            }
        }
    }

    let mut collected_songs: Vec<ItunesTrack> = Vec::new();
    for id in artist_ids {
        enforce_rate_limit().await;

        let id_str = id.to_string();
        let lookup_response = client
            .get(ITUNES_LOOKUP_URL)
            .query(&[
                ("id", id_str.as_str()),
                ("country", storefront),
                ("entity", "song"),
                ("limit", "150"),
            ])
            .send()
            .await
            .map_err(|err| format!("Erreur réseau lookup Apple Music ({storefront}) : {err}"))?;

        if !lookup_response.status().is_success() {
            return Err(format!(
                "Lookup Apple Music ({storefront}) a répondu avec le statut HTTP {}",
                lookup_response.status()
            ));
        }

        let lookup_payload: ItunesSearchResponse = lookup_response
            .json()
            .await
            .map_err(|err| format!("Réponse JSON lookup Apple Music invalide : {err}"))?;

        collected_songs.extend(filter_song_tracks(lookup_payload.results));
    }

    Ok(collected_songs)
}

async fn resolve_across_storefronts<F1, Fut1, F2, Fut2>(
    parsed: &ParsedTrackQuery,
    mut fetch_songs: F1,
    mut fetch_artist_songs: F2,
) -> Result<Option<(&'static str, ItunesTrack)>, String>
where
    F1: FnMut(String, &'static str) -> Fut1,
    Fut1: Future<Output = Result<Vec<ItunesTrack>, String>>,
    F2: FnMut(String, &'static str) -> Fut2,
    Fut2: Future<Output = Result<Vec<ItunesTrack>, String>>,
{
    for &storefront in STOREFRONT_CASCADE {
        let candidates = fetch_songs(parsed.search_term.clone(), storefront).await?;
        if let Some(track) = pick_best_candidate(candidates, parsed) {
            return Ok(Some((storefront, track)));
        }
    }

    if let Some(artist) = parsed.artist_hint.as_deref() {
        for &storefront in [DEFAULT_STOREFRONT, FRENCH_FALLBACK_STOREFRONT].iter() {
            let candidates = fetch_artist_songs(artist.to_string(), storefront).await?;
            if let Some(track) = pick_best_candidate(candidates, parsed) {
                return Ok(Some((storefront, track)));
            }
        }
    }

    Ok(None)
}

fn build_lookup_result(
    parsed: ParsedTrackQuery,
    resolved: Option<(&'static str, ItunesTrack)>,
) -> MetadataLookupResult {
    if let Some((storefront, track)) = resolved {
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

        MetadataLookupResult {
            status: LookupStatus::Matched,
            query_used: parsed.search_term,
            storefront: Some(storefront.to_string()),
            metadata: Some(normalized),
            detected_keyword: None,
            reason: None,
        }
    } else {
        let reason = if parsed.artist_hint.is_none() {
            format!(
                "Requête « {} » sans artiste détecté — aucune correspondance fiable (US/UK/FR).",
                parsed.search_term
            )
        } else {
            format!(
                "Requête « {} » — aucune correspondance fiable trouvée sur Apple Music (US/UK/FR).",
                parsed.search_term
            )
        };

        MetadataLookupResult {
            status: LookupStatus::NotFound,
            query_used: parsed.search_term,
            storefront: None,
            metadata: None,
            detected_keyword: None,
            reason: Some(reason),
        }
    }
}

/// Recherche un morceau sur Apple Music (`US` -> `GB` -> `FR`, puis lookup catalogue artiste)
/// en respectant les mots-clés intouchables (§4.3) et le rate limiting anti-ban (§4.2).
pub async fn search_track(
    source_path: &Path,
    existing: &ExistingTrackTags,
) -> Result<MetadataLookupResult, String> {
    let file_name = source_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source_path.to_string_lossy().into_owned());

    // 1. Garde-fou absolu : mots-clés intouchables dans le nom de fichier ou le titre existant
    if let Some(kw) = detect_protected_keyword(&file_name)
        .or_else(|| existing.title.as_deref().and_then(detect_protected_keyword))
    {
        return Ok(MetadataLookupResult {
            status: LookupStatus::ProtectedVersion,
            query_used: file_name,
            storefront: None,
            metadata: None,
            detected_keyword: Some(kw.to_string()),
            reason: Some(format!(
                "Mot-clé protégé « {kw} » détecté : Apple Music bloqué pour éviter d'écraser par une version studio (yt-dlp / révision manuelle)."
            )),
        });
    }

    let parsed = build_search_query(source_path, existing);
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

    let resolved = resolve_across_storefronts(
        &parsed,
        |term, storefront| async move { query_storefront(&term, storefront).await },
        |artist, storefront| async move { query_artist_catalog(&artist, storefront).await },
    )
    .await?;

    Ok(build_lookup_result(parsed, resolved))
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

    fn sample_track(artist: &str, title: &str, album: &str, genre: &str) -> ItunesTrack {
        ItunesTrack {
            wrapper_type: Some("track".into()),
            kind: Some("song".into()),
            track_name: Some(title.into()),
            artist_name: Some(artist.into()),
            collection_artist_name: Some(artist.into()),
            collection_name: Some(album.into()),
            track_number: Some(5),
            track_count: Some(15),
            disc_number: Some(1),
            disc_count: Some(1),
            release_date: Some("2024-01-19T12:00:00Z".into()),
            primary_genre_name: Some(genre.into()),
            artwork_url100: Some(
                "https://is1-ssl.mzstatic.com/image/thumb/test/100x100bb.jpg".into(),
            ),
        }
    }

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
                Path::new("01 - The Sims Theme (Junkie XL Remix).mp3"),
                &ExistingTrackTags::default(),
            ))
            .expect("should return ProtectedVersion without network call");

        assert_eq!(result.status, LookupStatus::ProtectedVersion);
        assert_eq!(result.detected_keyword.as_deref(), Some("remix"));
        assert!(result.metadata.is_none());
    }

    #[test]
    fn scores_exact_match_above_karaoke_or_live() {
        let studio = sample_track(
            "Daft Punk",
            "Get Lucky (feat. Pharrell Williams & Nile Rodgers)",
            "Random Access Memories",
            "Electronic",
        );
        let karaoke = ItunesTrack {
            track_name: Some("Get Lucky (Karaoke Version)".into()),
            ..studio.clone()
        };

        assert!(score_candidate(&studio, Some("Daft Punk"), "Get Lucky", None) > 0.8);
        assert!(score_candidate(&karaoke, Some("Daft Punk"), "Get Lucky", None) < 0.0);
    }

    #[test]
    fn cascades_from_us_and_gb_to_fr_and_normalizes_french_genre() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        let parsed = build_search_query(
            Path::new("/Users/jujutravail/Music/Clara Luciani - La Grenade.mp3"),
            &ExistingTrackTags::default(),
        );
        let mut visited: Vec<&'static str> = Vec::new();

        let resolved = rt
            .block_on(resolve_across_storefronts(
                &parsed,
                |_term, storefront| {
                    visited.push(storefront);
                    let res = if storefront == "FR" {
                        vec![sample_track(
                            "Clara Luciani",
                            "La grenade",
                            "Sainte-Victoire",
                            "Variété française",
                        )]
                    } else {
                        Vec::new()
                    };
                    async move { Ok(res) }
                },
                |_artist, _storefront| async { Ok(Vec::new()) },
            ))
            .expect("cascade should succeed");

        let result = build_lookup_result(parsed, resolved);
        assert_eq!(visited, vec!["US", "GB", "FR"]);
        assert_eq!(result.status, LookupStatus::Matched);
        assert_eq!(result.storefront.as_deref(), Some("FR"));
        let meta = result.metadata.expect("metadata expected");
        assert_eq!(meta.genre.as_deref(), Some("French Pop"));
    }

    #[test]
    fn falls_back_to_artist_catalog_lookup_for_short_titles_like_t_mimi() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        let parsed = build_search_query(
            Path::new("/Users/jujutravail/Music/Shay/Pourvu qu'il pleuve/05 T mimi.mp3"),
            &ExistingTrackTags::default(),
        );

        let resolved = rt
            .block_on(resolve_across_storefronts(
                &parsed,
                |_term, _storefront| async { Ok(Vec::new()) },
                |artist, storefront| async move {
                    if artist == "Shay" && storefront == "US" {
                        Ok(vec![
                            sample_track("Shay", "Jolie Go", "Pourvu qu'il pleuve", "Hip-Hop/Rap"),
                            sample_track("Shay", "T mimi", "Pourvu qu'il pleuve", "Hip-Hop/Rap"),
                        ])
                    } else {
                        Ok(Vec::new())
                    }
                },
            ))
            .expect("artist catalog fallback should succeed");

        let result = build_lookup_result(parsed, resolved);
        assert_eq!(result.status, LookupStatus::Matched);
        let meta = result.metadata.expect("metadata expected");
        assert_eq!(meta.artist, "Shay");
        assert_eq!(meta.title, "T mimi");
        assert_eq!(meta.album, "Pourvu qu'il pleuve");
        assert_eq!(meta.genre.as_deref(), Some("Hip-Hop/Rap"));
    }

    #[test]
    fn rejects_ambiguous_short_title_without_artist_hint() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        let parsed = build_search_query(
            Path::new("/Users/jujutravail/Downloads/05 T mimi.mp3"),
            &ExistingTrackTags::default(),
        );

        let resolved = rt
            .block_on(resolve_across_storefronts(
                &parsed,
                |_term, _storefront| async {
                    Ok(vec![sample_track(
                        "René Reulings",
                        "'t Mimi",
                        "'t Mimi - Single",
                        "Big Band",
                    )])
                },
                |_artist, _storefront| async { Ok(Vec::new()) },
            ))
            .expect("should reject false positive");

        let result = build_lookup_result(parsed, resolved);
        assert_eq!(result.status, LookupStatus::NotFound);
        assert!(result.metadata.is_none());
        let reason = result.reason.expect("reason expected");
        assert!(reason.contains("T mimi"));
        assert!(reason.contains("sans artiste détecté"));
        assert!(reason.contains("US/UK/FR"));
    }
}

//! Cascade de sources de métadonnées (SPECIFICATIONS.md §4.2).
//!
//! Ordre de priorité :
//! 1. Apple Music (iTunes Search API, région forcée US -> UK -> FR + lookup artiste, rate-limiting 1-2s)
//! 2. YouTube & SoundCloud via `yt-dlp` (remixes, lives, mashups, versions protégées et fallback)
//! 3. Fallback pochette complémentaire
//! 4. Genius API (paroles, optionnel via `.env`)
//! 5. Discogs API (V2 / rips CD physiques, optionnel via `.env`)

pub mod apple_music;
pub mod discogs;
pub mod genius;
pub mod ytdlp;

use std::collections::HashMap;
use std::path::Path;

use crate::models::{ExistingTrackTags, LookupStatus, MetadataLookupResult, TrackCandidate};
use crate::tagger::rules::normalize_for_match;

const MAX_COMBINED_CANDIDATES: usize = 6;

fn apply_ytdlp_fallback(
    lookup: &mut MetadataLookupResult,
    ytdlp_result: Result<Vec<TrackCandidate>, ytdlp::YtDlpError>,
) {
    match lookup.status {
        LookupStatus::Matched => {}
        LookupStatus::ProtectedVersion => {
            let kw = lookup
                .detected_keyword
                .as_deref()
                .unwrap_or("protégé")
                .to_string();

            match ytdlp_result {
                Ok(cands) if !cands.is_empty() => {
                    let count = cands.len();
                    lookup.candidates = cands;
                    lookup.reason = Some(format!(
                        "Mot-clé protégé « {kw} » détecté : Apple Music bloqué · {count} suggestion(s) YouTube / SoundCloud trouvée(s) via yt-dlp."
                    ));
                }
                Ok(_) => {
                    lookup.reason = Some(format!(
                        "Mot-clé protégé « {kw} » détecté : Apple Music bloqué · aucune suggestion YouTube / SoundCloud trouvée (révision manuelle)."
                    ));
                }
                Err(ytdlp::YtDlpError::NotInstalled) => {
                    lookup.reason = Some(format!(
                        "Mot-clé protégé « {kw} » détecté : Apple Music bloqué · yt-dlp absent du PATH (révision manuelle requise)."
                    ));
                }
                Err(err) => {
                    lookup.reason = Some(format!(
                        "Mot-clé protégé « {kw} » détecté : Apple Music bloqué · {err}"
                    ));
                }
            }
        }
        LookupStatus::NotFound => {
            let base_reason = lookup
                .reason
                .clone()
                .unwrap_or_else(|| "Aucune correspondance fiable trouvée sur Apple Music.".to_string());

            match ytdlp_result {
                Ok(yt_cands) if !yt_cands.is_empty() => {
                    let yt_count = yt_cands.len();
                    let mut merged: HashMap<(String, String, String), TrackCandidate> =
                        HashMap::new();

                    for cand in yt_cands.into_iter().chain(lookup.candidates.drain(..)) {
                        let key = (
                            normalize_for_match(&cand.artist),
                            normalize_for_match(&cand.title),
                            cand.platform
                                .clone()
                                .unwrap_or_else(|| cand.storefront.clone()),
                        );
                        match merged.get_mut(&key) {
                            Some(existing) if existing.confidence_score >= cand.confidence_score => {
                            }
                            Some(existing) => *existing = cand,
                            None => {
                                merged.insert(key, cand);
                            }
                        }
                    }

                    let mut ranked: Vec<TrackCandidate> = merged.into_values().collect();
                    ranked.sort_by(|a, b| b.confidence_score.total_cmp(&a.confidence_score));
                    ranked.truncate(MAX_COMBINED_CANDIDATES);

                    lookup.candidates = ranked;
                    lookup.reason = Some(format!(
                        "Aucune correspondance fiable sur Apple Music (US/UK/FR) · {yt_count} suggestion(s) YouTube / SoundCloud trouvée(s) via yt-dlp."
                    ));
                }
                Ok(_) => {}
                Err(ytdlp::YtDlpError::NotInstalled) => {
                    lookup.reason = Some(format!(
                        "{base_reason} (yt-dlp absent du PATH — fallback YouTube / SoundCloud indisponible)."
                    ));
                }
                Err(err) => {
                    lookup.reason = Some(format!("{base_reason} ({err})"));
                }
            }
        }
    }
}

/// Résolution unifiée d'une piste :
/// 1. Apple Music (`US -> GB -> FR` + lookup catalogue artiste)
/// 2. Fallback automatique `yt-dlp` (YouTube & SoundCloud) sur mots-clés protégés ou échec Apple Music.
pub async fn resolve_track_metadata(
    source_path: &Path,
    existing: &ExistingTrackTags,
) -> Result<MetadataLookupResult, String> {
    let mut lookup = apple_music::search_track(source_path, existing).await?;

    if lookup.status != LookupStatus::Matched && !lookup.query_used.trim().is_empty() {
        let yt_res = ytdlp::search_candidates(&lookup.query_used).await;
        apply_ytdlp_fallback(&mut lookup, yt_res);
    }

    Ok(lookup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MetadataSource;

    fn sample_yt_candidate() -> TrackCandidate {
        TrackCandidate {
            title: "Get Lucky (Remix)".to_string(),
            artist: "Daft Punk".to_string(),
            album_artist: "Daft Punk".to_string(),
            album: "Get Lucky (Remix)".to_string(),
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            year: Some(2013),
            genre: None,
            artwork_url: None,
            is_compilation: false,
            storefront: "YouTube".to_string(),
            confidence_score: 0.91,
            source: Some(MetadataSource::YtDlp),
            platform: Some("YouTube".to_string()),
            duration_secs: Some(633),
        }
    }

    #[test]
    fn enriches_protected_version_with_ytdlp_or_missing_binary_notice() {
        let mut protected = MetadataLookupResult {
            status: LookupStatus::ProtectedVersion,
            query_used: "Daft Punk Get Lucky Remix".to_string(),
            storefront: None,
            metadata: None,
            candidates: Vec::new(),
            reason: Some("Mot-clé protégé".to_string()),
            detected_keyword: Some("remix".to_string()),
        };

        apply_ytdlp_fallback(&mut protected, Ok(vec![sample_yt_candidate()]));
        assert_eq!(protected.candidates.len(), 1);
        assert_eq!(protected.candidates[0].platform.as_deref(), Some("YouTube"));
        assert!(
            protected
                .reason
                .as_deref()
                .unwrap()
                .contains("YouTube / SoundCloud")
        );

        let mut missing_bin = MetadataLookupResult {
            status: LookupStatus::ProtectedVersion,
            query_used: "Daft Punk Get Lucky Remix".to_string(),
            storefront: None,
            metadata: None,
            candidates: Vec::new(),
            reason: Some("Mot-clé protégé".to_string()),
            detected_keyword: Some("remix".to_string()),
        };
        apply_ytdlp_fallback(&mut missing_bin, Err(ytdlp::YtDlpError::NotInstalled));
        assert!(missing_bin.candidates.is_empty());
        assert!(missing_bin.reason.as_deref().unwrap().contains("PATH"));
    }
}

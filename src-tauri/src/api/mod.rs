//! Cascade de sources de métadonnées multi-sources (SPECIFICATIONS.md §4.2).
//!
//! Ordre de priorité en mode `Auto` :
//! - **Versions protégées** (`Live`, `Remix`, `Mashup`, `Sped Up`, `Chopped`, `Acoustic`,
//!   `Instrumental`, `Slowed`, `Reverb`, `Nightcore`) : routage prioritaire `yt-dlp` (YouTube / SoundCloud).
//! - **Pistes classiques** :
//!   1. Apple Music (`US -> GB -> FR` + lookup catalogue artiste)
//!   2. Deezer (API publique, `cover_xl` 1000×1000)
//!   3. MusicBrainz + Cover Art Archive (API publique, 1 req/s)
//!   4. Discogs (optionnel via `DISCOGS_TOKEN` dans `.env`)
//!   5. YouTube / SoundCloud (`yt-dlp`)

pub mod apple_music;
pub mod deezer;
pub mod discogs;
pub mod genius;
pub mod musicbrainz;
pub mod ytdlp;

use std::collections::HashMap;
use std::path::Path;

use crate::models::{
    ExistingTrackTags, LookupStatus, MetadataLookupResult, SearchSource, TrackCandidate,
};
use crate::tagger::rules::{detect_protected_keyword, normalize_for_match};

const MAX_COMBINED_CANDIDATES: usize = 6;

const PROTECTED_AUTO_CASCADE: &[SearchSource] = &[SearchSource::Youtube, SearchSource::Soundcloud];

const CLASSIC_AUTO_CASCADE: &[SearchSource] = &[
    SearchSource::AppleMusic,
    SearchSource::Deezer,
    SearchSource::MusicBrainz,
    SearchSource::Discogs,
    SearchSource::Youtube,
    SearchSource::Soundcloud,
];

/// Retourne l'ordre officiel de cascade `Auto` selon que la piste contient un mot-clé protégé ou non.
pub fn auto_cascade_order(is_protected: bool) -> &'static [SearchSource] {
    if is_protected {
        PROTECTED_AUTO_CASCADE
    } else {
        CLASSIC_AUTO_CASCADE
    }
}

fn merge_and_rank_candidates(
    existing: Vec<TrackCandidate>,
    incoming: Vec<TrackCandidate>,
    limit: usize,
) -> Vec<TrackCandidate> {
    let mut merged: HashMap<(String, String, String), TrackCandidate> = HashMap::new();

    for cand in existing.into_iter().chain(incoming) {
        let key = (
            normalize_for_match(&cand.artist),
            normalize_for_match(&cand.title),
            cand.platform
                .clone()
                .unwrap_or_else(|| cand.storefront.clone()),
        );
        match merged.get_mut(&key) {
            Some(prev) if prev.confidence_score >= cand.confidence_score => {}
            Some(prev) => *prev = cand,
            None => {
                merged.insert(key, cand);
            }
        }
    }

    let mut ranked: Vec<TrackCandidate> = merged.into_values().collect();
    ranked.sort_by(|a, b| b.confidence_score.total_cmp(&a.confidence_score));
    ranked.truncate(limit);
    ranked
}

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
                        "Mot-clé protégé « {kw} » détecté : {count} suggestion(s) YouTube / SoundCloud trouvée(s)."
                    ));
                }
                Ok(_) => {
                    lookup.reason = Some(format!(
                        "Mot-clé protégé « {kw} » détecté : aucune suggestion YouTube / SoundCloud trouvée (révision manuelle)."
                    ));
                }
                Err(ytdlp::YtDlpError::NotInstalled) => {
                    lookup.reason = Some(format!(
                        "Mot-clé protégé « {kw} » détecté : yt-dlp absent du PATH (révision manuelle requise)."
                    ));
                }
                Err(err) => {
                    lookup.reason = Some(format!(
                        "Mot-clé protégé « {kw} » détecté : {err}"
                    ));
                }
            }
        }
        LookupStatus::NotFound => {
            let base_reason = lookup
                .reason
                .clone()
                .unwrap_or_else(|| "Aucune correspondance fiable trouvée.".to_string());

            match ytdlp_result {
                Ok(yt_cands) if !yt_cands.is_empty() => {
                    let yt_count = yt_cands.len();
                    let existing = std::mem::take(&mut lookup.candidates);
                    lookup.candidates =
                        merge_and_rank_candidates(existing, yt_cands, MAX_COMBINED_CANDIDATES);
                    lookup.reason = Some(format!(
                        "Suggestions alternatives trouvées (+{yt_count} YouTube / SoundCloud)."
                    ));
                }
                Ok(_) => {}
                Err(ytdlp::YtDlpError::NotInstalled) => {
                    if lookup.candidates.is_empty() {
                        lookup.reason = Some(format!(
                            "{base_reason} (yt-dlp absent du PATH)."
                        ));
                    }
                }
                Err(err) => {
                    if lookup.candidates.is_empty() {
                        lookup.reason = Some(format!("{base_reason} ({err})"));
                    }
                }
            }
        }
    }
}

/// Résolution unifiée en mode `Auto` :
/// - Mots-clés protégés : `yt-dlp` (YouTube / SoundCloud) en priorité.
/// - Pistes classiques : Apple Music (`US -> GB -> FR`) -> Deezer -> MusicBrainz -> Discogs (si token) -> `yt-dlp`.
pub async fn resolve_track_metadata(
    source_path: &Path,
    existing: &ExistingTrackTags,
    app: Option<&tauri::AppHandle>,
) -> Result<MetadataLookupResult, String> {
    let mut lookup = apple_music::search_track(source_path, existing).await?;

    if lookup.status == LookupStatus::Matched || lookup.query_used.trim().is_empty() {
        return Ok(lookup);
    }

    if lookup.status == LookupStatus::ProtectedVersion {
        let yt_res = ytdlp::search_candidates(&lookup.query_used).await;
        apply_ytdlp_fallback(&mut lookup, yt_res);
        return Ok(lookup);
    }

    // Cascade multi-sources pour pistes classiques non identifiées sur Apple Music :
    // Deezer -> MusicBrainz -> Discogs (optionnel) -> yt-dlp
    let query = lookup.query_used.clone();
    let mut found_sources: Vec<&str> = Vec::new();

    if let Ok(deezer_cands) = deezer::search_candidates(&query).await
        && !deezer_cands.is_empty()
    {
        found_sources.push("Deezer");
        let existing_cands = std::mem::take(&mut lookup.candidates);
        lookup.candidates =
            merge_and_rank_candidates(existing_cands, deezer_cands, MAX_COMBINED_CANDIDATES);
    }

    if lookup.candidates.len() < 3
        && let Ok(mb_cands) = musicbrainz::search_candidates(&query).await
        && !mb_cands.is_empty()
    {
        found_sources.push("MusicBrainz");
        let existing_cands = std::mem::take(&mut lookup.candidates);
        lookup.candidates =
            merge_and_rank_candidates(existing_cands, mb_cands, MAX_COMBINED_CANDIDATES);
    }

    if lookup.candidates.len() < 4
        && let Ok(discogs_cands) = discogs::search_candidates(&query, app, false).await
        && !discogs_cands.is_empty()
    {
        found_sources.push("Discogs");
        let existing_cands = std::mem::take(&mut lookup.candidates);
        lookup.candidates =
            merge_and_rank_candidates(existing_cands, discogs_cands, MAX_COMBINED_CANDIDATES);
    }

    if !found_sources.is_empty() {
        lookup.reason = Some(format!(
            "Apple Music sans correspondance directe · suggestions trouvées sur {}.",
            found_sources.join(", ")
        ));
    }

    if lookup.candidates.is_empty() {
        let yt_res = ytdlp::search_candidates(&query).await;
        apply_ytdlp_fallback(&mut lookup, yt_res);
    }

    Ok(lookup)
}

/// Recherche rapide unifiée pour le sélecteur à 7 options (`Auto`, `Apple Music`, `Deezer`,
/// `YouTube`, `SoundCloud`, `MusicBrainz`, `Discogs`).
pub async fn search_candidates_by_source(
    source: SearchSource,
    query: &str,
    app: Option<&tauri::AppHandle>,
) -> Result<Vec<TrackCandidate>, String> {
    let cleaned = query.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    match source {
        SearchSource::AppleMusic => apple_music::search_candidates_freeform(&cleaned).await,
        SearchSource::Deezer => deezer::search_candidates(&cleaned).await,
        SearchSource::Youtube => ytdlp::search_youtube_candidates(&cleaned)
            .await
            .map_err(|err| err.to_string()),
        SearchSource::Soundcloud => ytdlp::search_soundcloud_candidates(&cleaned)
            .await
            .map_err(|err| err.to_string()),
        SearchSource::MusicBrainz => musicbrainz::search_candidates(&cleaned).await,
        SearchSource::Discogs => discogs::search_candidates(&cleaned, app, true).await,
        SearchSource::Auto => {
            let is_protected = detect_protected_keyword(&cleaned).is_some();
            if is_protected {
                return ytdlp::search_candidates(&cleaned)
                    .await
                    .map_err(|err| err.to_string());
            }

            let mut results: Vec<TrackCandidate> = Vec::new();
            let mut last_err: Option<String> = None;

            match apple_music::search_candidates_freeform(&cleaned).await {
                Ok(cands) => results = merge_and_rank_candidates(results, cands, MAX_COMBINED_CANDIDATES),
                Err(e) => last_err = Some(e),
            }

            if results.len() < 3 {
                match deezer::search_candidates(&cleaned).await {
                    Ok(cands) => {
                        results = merge_and_rank_candidates(results, cands, MAX_COMBINED_CANDIDATES)
                    }
                    Err(e) if results.is_empty() => last_err = Some(e),
                    Err(_) => {}
                }
            }

            if results.len() < 3
                && let Ok(cands) = musicbrainz::search_candidates(&cleaned).await
            {
                results = merge_and_rank_candidates(results, cands, MAX_COMBINED_CANDIDATES);
            }

            if results.len() < 3
                && let Ok(cands) = discogs::search_candidates(&cleaned, app, false).await
            {
                results = merge_and_rank_candidates(results, cands, MAX_COMBINED_CANDIDATES);
            }

            if results.is_empty() {
                match ytdlp::search_candidates(&cleaned).await {
                    Ok(cands) => {
                        results = merge_and_rank_candidates(results, cands, MAX_COMBINED_CANDIDATES)
                    }
                    Err(e) if last_err.is_none() => last_err = Some(e.to_string()),
                    Err(_) => {}
                }
            }

            if results.is_empty()
                && let Some(err) = last_err
            {
                return Err(err);
            }

            Ok(results)
        }
    }
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
            bpm: None,
        }
    }

    #[test]
    fn enforces_auto_cascade_order_for_classic_and_protected_tracks() {
        assert_eq!(
            auto_cascade_order(true),
            &[SearchSource::Youtube, SearchSource::Soundcloud]
        );
        assert_eq!(
            auto_cascade_order(false),
            &[
                SearchSource::AppleMusic,
                SearchSource::Deezer,
                SearchSource::MusicBrainz,
                SearchSource::Discogs,
                SearchSource::Youtube,
                SearchSource::Soundcloud,
            ]
        );
    }

    #[test]
    fn separates_youtube_and_soundcloud_search_specs_in_ytdlp() {
        let yt = ytdlp::build_search_specs(ytdlp::YtDlpTarget::Youtube, "Daft Punk Remix");
        assert_eq!(yt.len(), 1);
        assert_eq!(yt[0].1, "YouTube");
        assert!(yt[0].0.starts_with("ytsearch"));

        let sc = ytdlp::build_search_specs(ytdlp::YtDlpTarget::Soundcloud, "Daft Punk Remix");
        assert_eq!(sc.len(), 1);
        assert_eq!(sc[0].1, "SoundCloud");
        assert!(sc[0].0.starts_with("scsearch"));
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

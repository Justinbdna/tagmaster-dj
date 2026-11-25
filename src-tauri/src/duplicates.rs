//! Détection intelligente de doublons dans la file audio (SPECIFICATIONS.md · Issue #14).
//!
//! Regroupe les pistes correspondant au même couple normalisé `Artiste + Titre`
//! (après identification ou par analyse du nom de fichier nettoyé) ou au même
//! nom de fichier nettoyé, en éliminant les suffixes techniques de copie
//! (`__2`, `_1`, `(1)`, `(2)`, ` - Copy`, ` copie`) tout en préservant les
//! versions distinctes protégées (`Live`, `Remix`, `Instrumental`, `Sped Up`, etc.).

use std::collections::HashMap;

use crate::models::{AudioTrackItem, DuplicateGroup};
use crate::tagger::rules::{
    clean_filename_stem, extract_primary_artist, normalize_for_match, split_artist_title,
    strip_duplicate_copy_suffix, strip_feat_from_title,
};

struct DuplicateSignature {
    key: String,
    display_label: String,
    match_reason: String,
}

fn format_quality_rank(ext: &str) -> u8 {
    match ext.trim().to_ascii_uppercase().as_str() {
        "FLAC" | "WAV" | "ALAC" | "AIFF" => 4,
        "M4A" | "AAC" => 3,
        "MP3" => 2,
        _ => 1,
    }
}

fn track_quality_tuple(track: &AudioTrackItem) -> (u8, u32, u64) {
    (
        format_quality_rank(&track.extension),
        track.bitrate_kbps.unwrap_or(0),
        track.size_bytes,
    )
}

fn compute_duplicate_signature(track: &AudioTrackItem) -> Option<DuplicateSignature> {
    if let Some(meta) = &track.metadata {
        let primary_artist = extract_primary_artist(&meta.artist);
        let clean_title = strip_duplicate_copy_suffix(&strip_feat_from_title(&meta.title));
        let norm_artist = normalize_for_match(&primary_artist);
        let norm_title = normalize_for_match(&clean_title);

        if !norm_artist.is_empty() && !norm_title.is_empty() {
            return Some(DuplicateSignature {
                key: format!("at::{norm_artist}::{norm_title}"),
                display_label: format!("{primary_artist} — {clean_title}"),
                match_reason: "Même artiste & titre identifiés".to_string(),
            });
        }
    }

    let cleaned_stem = clean_filename_stem(&track.file_name);
    if cleaned_stem.is_empty() {
        return None;
    }

    if let Some((artist, title)) = split_artist_title(&cleaned_stem) {
        let primary_artist = extract_primary_artist(&artist);
        let clean_title = strip_duplicate_copy_suffix(&strip_feat_from_title(&title));
        let norm_artist = normalize_for_match(&primary_artist);
        let norm_title = normalize_for_match(&clean_title);

        if !norm_artist.is_empty() && !norm_title.is_empty() {
            return Some(DuplicateSignature {
                key: format!("at::{norm_artist}::{norm_title}"),
                display_label: format!("{primary_artist} — {clean_title}"),
                match_reason: "Nom normalisé identique (variante / copie)".to_string(),
            });
        }
    }

    let norm_stem = normalize_for_match(&cleaned_stem);
    if norm_stem.is_empty() {
        return None;
    }

    Some(DuplicateSignature {
        key: format!("stem::{norm_stem}"),
        display_label: cleaned_stem,
        match_reason: "Nom de fichier nettoyé identique".to_string(),
    })
}

/// Détecte tous les groupes de doublons (>= 2 pistes partageant la même signature).
/// Les pistes de chaque groupe sont ordonnées par qualité décroissante (lossless > bitrate > taille).
pub fn detect_duplicate_groups(tracks: &[AudioTrackItem]) -> Vec<DuplicateGroup> {
    struct Bucket<'a> {
        display_label: String,
        match_reason: String,
        has_metadata_match: bool,
        first_seen_idx: usize,
        items: Vec<&'a AudioTrackItem>,
    }

    let mut buckets: HashMap<String, Bucket<'_>> = HashMap::new();

    for (idx, track) in tracks.iter().enumerate() {
        let Some(sig) = compute_duplicate_signature(track) else {
            continue;
        };

        let is_meta = track.metadata.is_some();
        let entry = buckets.entry(sig.key).or_insert_with(|| Bucket {
            display_label: sig.display_label.clone(),
            match_reason: sig.match_reason.clone(),
            has_metadata_match: is_meta,
            first_seen_idx: idx,
            items: Vec::new(),
        });

        if is_meta && !entry.has_metadata_match {
            entry.display_label = sig.display_label;
            entry.match_reason = sig.match_reason;
            entry.has_metadata_match = true;
        }

        entry.items.push(track);
    }

    let mut grouped: Vec<(usize, DuplicateGroup)> = buckets
        .into_iter()
        .filter(|(_, bucket)| bucket.items.len() >= 2)
        .map(|(group_key, mut bucket)| {
            bucket.items.sort_by(|a, b| {
                track_quality_tuple(b)
                    .cmp(&track_quality_tuple(a))
                    .then_with(|| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()))
            });

            let track_ids = bucket.items.into_iter().map(|t| t.id.clone()).collect();
            (
                bucket.first_seen_idx,
                DuplicateGroup {
                    group_key,
                    display_label: bucket.display_label,
                    match_reason: bucket.match_reason,
                    track_ids,
                },
            )
        })
        .collect();

    grouped.sort_by_key(|(idx, _)| *idx);
    grouped.into_iter().map(|(_, group)| group).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MetadataSource, TrackMetadata, TrackStatus};

    fn mock_track(
        id: &str,
        file_name: &str,
        ext: &str,
        size_bytes: u64,
        bitrate_kbps: Option<u32>,
        metadata: Option<(&str, &str)>,
    ) -> AudioTrackItem {
        AudioTrackItem {
            id: id.to_string(),
            source_path: format!("/music/{file_name}"),
            file_name: file_name.to_string(),
            parent_folder: "Imports".to_string(),
            extension: ext.to_string(),
            size_bytes,
            has_protected_keyword: false,
            status: if metadata.is_some() {
                TrackStatus::Completed
            } else {
                TrackStatus::Pending
            },
            metadata: metadata.map(|(artist, title)| TrackMetadata {
                title: title.to_string(),
                artist: artist.to_string(),
                album_artist: artist.to_string(),
                album: "Album Test".to_string(),
                track_number: None,
                track_total: None,
                disc_number: None,
                disc_total: None,
                year: Some(2024),
                genre: Some("Pop".to_string()),
                artwork_url: None,
                is_compilation: false,
                source: Some(MetadataSource::AppleMusic),
                lyrics: None,
                synced_lyrics: None,
            }),
            exported_path: None,
            local_artwork_path: None,
            review_reason: None,
            candidates: Vec::new(),
            last_query_used: None,
            duration_secs: Some(210),
            bitrate_kbps,
            sample_rate_hz: Some(44_100),
        }
    }

    #[test]
    fn detects_copy_suffix_variants_and_sorts_highest_quality_first() {
        let tracks = vec![
            mock_track(
                "mp3_low",
                "Bad Gyal - Chulo__2.mp3",
                "MP3",
                4_000_000,
                Some(128),
                None,
            ),
            mock_track(
                "flac_hi",
                "Bad Gyal - Chulo (1).flac",
                "FLAC",
                32_000_000,
                Some(1024),
                None,
            ),
            mock_track(
                "mp3_320",
                "01. Bad Gyal - Chulo_1.mp3",
                "MP3",
                9_000_000,
                Some(320),
                None,
            ),
        ];

        let groups = detect_duplicate_groups(&tracks);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].track_ids, vec!["flac_hi", "mp3_320", "mp3_low"]);
    }

    #[test]
    fn detects_duplicates_across_identified_and_pending_tracks_ignoring_feat_and_accents() {
        let tracks = vec![
            mock_track(
                "identified",
                "random_download_001.mp3",
                "MP3",
                8_200_000,
                Some(320),
                Some(("Beyoncé", "Drunk in Love (feat. JAY-Z)")),
            ),
            mock_track(
                "pending_copy",
                "Beyonce - Drunk in Love (1).m4a",
                "M4A",
                9_500_000,
                Some(256),
                None,
            ),
        ];

        let groups = detect_duplicate_groups(&tracks);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].track_ids.len(), 2);
        assert_eq!(groups[0].display_label, "Beyoncé — Drunk in Love");
    }

    #[test]
    fn does_not_merge_studio_version_with_protected_remix_or_live_version() {
        let tracks = vec![
            mock_track(
                "studio",
                "Daft Punk - Get Lucky.mp3",
                "MP3",
                9_000_000,
                Some(320),
                Some(("Daft Punk", "Get Lucky")),
            ),
            mock_track(
                "remix",
                "Daft Punk - Get Lucky (Remix).mp3",
                "MP3",
                11_000_000,
                Some(320),
                Some(("Daft Punk", "Get Lucky (Remix)")),
            ),
            mock_track(
                "live",
                "Daft Punk - Get Lucky (Live).mp3",
                "MP3",
                12_000_000,
                Some(320),
                None,
            ),
        ];

        let groups = detect_duplicate_groups(&tracks);
        assert!(
            groups.is_empty(),
            "Studio, Remix, and Live versions must remain distinct"
        );
    }
}

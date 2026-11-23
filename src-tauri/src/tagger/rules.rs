//! Règles métier strictes de tagging (SPECIFICATIONS.md §4.1 & §4.3) :
//! - Featurings normalisés dans le titre (`Chulo pt.2 (feat. Tokischa & Young Miko)`)
//!   et `Album Artist` verrouillé sur l'artiste principal unique.
//! - Singles : `Album` reprend le nom du single.
//! - Compilations / BO : `Album Artist` = `Various Artists`, vrai interprète conservé dans `Artist`.
//! - Multi-disques : `Disc Number` et `Disc Total` conservés (`1/2`, `2/2`).
//! - Mots-clés intouchables : empêcher Apple Music d'écraser une version alternative.

use std::sync::OnceLock;

use regex::Regex;

use crate::models::{MetadataSource, TrackMetadata};

pub const PROTECTED_KEYWORDS: &[&str] = &[
    "live",
    "acoustic",
    "sped up",
    "chopped",
    "instrumental",
    "remix",
    "mashup",
];

pub const DIRTY_TAG_PATTERNS: &[&str] = &[
    "mediahuman",
    "free-mp3",
    "y2mate",
    "mp3juices",
    "soundcloud downloader",
    "youtube-dl",
    "yt-dlp",
    "http://",
    "https://",
    "www.",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrackQuery {
    pub artist_hint: Option<String>,
    pub title_hint: String,
    pub search_term: String,
}

#[derive(Debug, Clone)]
pub struct RawMetadataInput {
    pub title: String,
    pub artist: String,
    pub collection_artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub artwork_url: Option<String>,
    pub is_compilation: bool,
    pub source: MetadataSource,
}

fn whitespace_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"))
}

fn leading_track_num_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*(?:\d{1,2}\s*-\s*)?\d{1,3}\s*(?:[.\-_)\]]\s*|\s+-\s+|\s+)")
            .expect("valid track prefix regex")
    })
}

fn noise_brackets_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)[\(\[](?:official\s*(?:music\s*)?(?:video|audio|lyric\s*video|visualizer)|lyrics?|audio|hd|hq|4k|320\s*kbps|explicit|clean|music\s*video|clip\s*officiel|visualizer)[\)\]]",
        )
        .expect("valid noise regex")
    })
}

fn title_feat_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[]?\s*(?:feat\.?|ft\.?|featuring)\s+([^\)\]]+)[\)\]]?")
            .expect("valid feat regex")
    })
}

fn artist_feat_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(.+?)\s+(?:feat\.?|ft\.?|featuring)\s+(.+)$")
            .expect("valid artist feat regex")
    })
}

fn single_suffix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\s*-\s*(?:single|ep)\s*$").expect("valid single suffix regex"))
}

fn collapse_spaces(input: &str) -> String {
    whitespace_re().replace_all(input.trim(), " ").into_owned()
}

/// Retourne le premier mot-clé intouchable détecté (avec limites de mots pour éviter les faux positifs
/// comme "Deliver" pour "live" ou "Olive" pour "live").
pub fn detect_protected_keyword(input: &str) -> Option<&'static str> {
    let lower = input.to_lowercase();
    for &kw in PROTECTED_KEYWORDS {
        let pattern = format!(r"(?i)(?:^|[^a-z0-9]){}(?:$|[^a-z0-9])", regex::escape(kw));
        if let Ok(re) = Regex::new(&pattern)
            && re.is_match(&lower)
        {
            return Some(kw);
        }
    }
    None
}

pub fn contains_protected_keyword(input: &str) -> bool {
    detect_protected_keyword(input).is_some()
}

pub fn looks_like_dirty_tag(value: &str) -> bool {
    let lower = value.to_lowercase();
    DIRTY_TAG_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Extrait une requête de recherche propre à partir du nom de fichier (et optionnellement des tags existants).
pub fn build_search_query(
    file_name: &str,
    existing_artist: Option<&str>,
    existing_title: Option<&str>,
) -> ParsedTrackQuery {
    let clean_existing_artist = existing_artist
        .map(collapse_spaces)
        .filter(|s| !s.is_empty() && !looks_like_dirty_tag(s));
    let clean_existing_title = existing_title
        .map(|s| collapse_spaces(&noise_brackets_re().replace_all(s, "")))
        .filter(|s| !s.is_empty() && !looks_like_dirty_tag(s));

    if let (Some(artist), Some(title)) = (clean_existing_artist, clean_existing_title) {
        let primary = extract_primary_artist(&artist);
        let base_title = strip_feat_from_title(&title);
        return ParsedTrackQuery {
            search_term: collapse_spaces(&format!("{primary} {base_title}")),
            artist_hint: Some(primary),
            title_hint: title,
        };
    }

    // Fallback sur le nom de fichier sans extension
    let stem = file_name
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(file_name);

    let without_noise = noise_brackets_re().replace_all(stem, " ");
    let without_underscores = without_noise.replace('_', " ");
    let without_prefix = leading_track_num_re().replace(&without_underscores, "");
    let cleaned = collapse_spaces(&without_prefix);

    if let Some((left, right)) = cleaned.split_once(" - ") {
        let artist = collapse_spaces(left);
        let title = collapse_spaces(right);
        if !artist.is_empty() && !title.is_empty() {
            let primary = extract_primary_artist(&artist);
            let base_title = strip_feat_from_title(&title);
            return ParsedTrackQuery {
                search_term: collapse_spaces(&format!("{primary} {base_title}")),
                artist_hint: Some(primary),
                title_hint: title,
            };
        }
    }

    let base_title = strip_feat_from_title(&cleaned);
    ParsedTrackQuery {
        search_term: if base_title.is_empty() {
            cleaned.clone()
        } else {
            base_title
        },
        artist_hint: None,
        title_hint: cleaned,
    }
}

fn strip_feat_from_title(title: &str) -> String {
    collapse_spaces(&title_feat_re().replace_all(title, ""))
}

fn normalize_feat_list(raw: &str) -> String {
    let cleaned = collapse_spaces(raw)
        .trim_matches(|c: char| matches!(c, '(' | ')' | '[' | ']' | '-' | ','))
        .trim()
        .to_string();

    // Harmoniser les séparateurs multiples vers ", " et " & " avant le dernier artiste
    let parts: Vec<String> = cleaned
        .split([',', '&', ';'])
        .map(|p| collapse_spaces(p.trim_start_matches("and ").trim_start_matches("et ")))
        .filter(|p| !p.is_empty())
        .collect();

    match parts.len() {
        0 => cleaned,
        1 => parts[0].clone(),
        2 => format!("{} & {}", parts[0], parts[1]),
        _ => {
            let last = parts.last().cloned().unwrap_or_default();
            let head = parts[..parts.len() - 1].join(", ");
            format!("{head} & {last}")
        }
    }
}

pub fn extract_primary_artist(artist: &str) -> String {
    let trimmed = collapse_spaces(artist);
    if let Some(caps) = artist_feat_re().captures(&trimmed) {
        return collapse_spaces(caps.get(1).map(|m| m.as_str()).unwrap_or(&trimmed));
    }

    // Si plusieurs artistes séparés par ", " ou " & ", verrouiller sur l'artiste principal unique
    for sep in [", ", " & ", " x ", " X ", " / "] {
        if let Some((first, _)) = trimmed.split_once(sep) {
            let candidate = collapse_spaces(first);
            if !candidate.is_empty() {
                return candidate;
            }
        }
    }

    trimmed
}

fn detect_compilation(raw: &RawMetadataInput) -> bool {
    if raw.is_compilation {
        return true;
    }
    if let Some(col_artist) = &raw.collection_artist {
        let lower = col_artist.to_lowercase();
        if lower == "various artists" || lower == "multi-interprètes" || lower == "compilation" {
            return true;
        }
    }
    if let Some(album) = &raw.album {
        let lower = album.to_lowercase();
        if lower.contains("original motion picture soundtrack")
            || lower.contains("original soundtrack")
            || lower.contains("(ost)")
        {
            return true;
        }
    }
    false
}

/// Applique intégralement les règles strictes de tagging (§4.3) sur une piste brute.
pub fn normalize_track_metadata(raw: RawMetadataInput) -> TrackMetadata {
    let mut artist = collapse_spaces(&raw.artist);
    let mut feat_artists: Vec<String> = Vec::new();

    // 1. Extraire un éventuel feat. présent dans le champ Artist
    if let Some(caps) = artist_feat_re().captures(&artist.clone()) {
        let main = collapse_spaces(caps.get(1).map(|m| m.as_str()).unwrap_or(&artist));
        let featured = collapse_spaces(caps.get(2).map(|m| m.as_str()).unwrap_or(""));
        artist = main;
        if !featured.is_empty() {
            feat_artists.push(featured);
        }
    }

    // 2. Extraire un éventuel feat. présent dans le champ Title et le normaliser
    let mut base_title = collapse_spaces(&raw.title);
    if let Some(caps) = title_feat_re().captures(&base_title.clone()) {
        let featured = collapse_spaces(caps.get(1).map(|m| m.as_str()).unwrap_or(""));
        base_title = strip_feat_from_title(&base_title);
        if !featured.is_empty()
            && !feat_artists
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&featured))
        {
            feat_artists.push(featured);
        }
    }

    let final_title = if feat_artists.is_empty() {
        base_title.clone()
    } else {
        let normalized_feats = normalize_feat_list(&feat_artists.join(", "));
        format!("{base_title} (feat. {normalized_feats})")
    };

    // 3. Compilation / BO vs Album Artist verrouillé sur l'artiste principal unique
    let is_compilation = detect_compilation(&raw);
    let album_artist = if is_compilation {
        "Various Artists".to_string()
    } else if let Some(col_artist) = raw.collection_artist.as_deref().map(collapse_spaces)
        && !col_artist.is_empty()
    {
        extract_primary_artist(&col_artist)
    } else {
        extract_primary_artist(&artist)
    };

    // 4. Singles : l'album reprend le nom du single (sans suffixe parasite "- Single")
    let raw_album = raw.album.as_deref().map(collapse_spaces).unwrap_or_default();
    let is_single_collection = raw_album.to_lowercase().ends_with("- single")
        || (raw.track_total == Some(1) && raw_album.is_empty());

    let album = if raw_album.is_empty() {
        base_title
    } else if is_single_collection {
        let stripped = collapse_spaces(&single_suffix_re().replace(&raw_album, ""));
        if stripped.is_empty() {
            base_title
        } else {
            stripped
        }
    } else {
        raw_album
    };

    // 5. Multi-disques : s'assurer que disc_number est rempli si disc_total > 1
    let (disc_number, disc_total) = match (raw.disc_number, raw.disc_total) {
        (Some(dn), Some(dt)) if dt > 1 => (Some(dn), Some(dt)),
        (None, Some(dt)) if dt > 1 => (Some(1), Some(dt)),
        (Some(dn), dt) => (Some(dn), dt),
        (None, dt) => (None, dt),
    };

    let genre = raw
        .genre
        .map(|g| collapse_spaces(&g))
        .filter(|g| !g.is_empty());

    TrackMetadata {
        title: final_title,
        artist,
        album_artist,
        album,
        track_number: raw.track_number,
        track_total: raw.track_total,
        disc_number,
        disc_total,
        year: raw.year,
        genre,
        artwork_url: raw.artwork_url,
        is_compilation,
        source: Some(raw.source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_protected_keywords_without_false_positives() {
        assert_eq!(
            detect_protected_keyword("01 - The Sims Theme (Junkie XL Remix).mp3"),
            Some("remix")
        );
        assert_eq!(
            detect_protected_keyword("Daft Punk - Alive (Live at Coachella).flac"),
            Some("live")
        );
        assert_eq!(
            detect_protected_keyword("SZA - Kill Bill (Sped Up).mp3"),
            Some("sped up")
        );
        // Faux positifs à éviter
        assert_eq!(detect_protected_keyword("Olivia Rodrigo - Drivers License.mp3"), None);
        assert_eq!(detect_protected_keyword("Adele - Deliverance.mp3"), None);
    }

    #[test]
    fn formats_featuring_and_locks_single_album_artist() {
        let meta = normalize_track_metadata(RawMetadataInput {
            title: "Chulo pt.2 [ft. Tokischa, Young Miko]".to_string(),
            artist: "Bad Gyal".to_string(),
            collection_artist: Some("Bad Gyal & Tokischa".to_string()),
            album: Some("La Joia".to_string()),
            track_number: Some(4),
            track_total: Some(15),
            disc_number: Some(1),
            disc_total: Some(1),
            year: Some(2024),
            genre: Some("Urbano latino".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: MetadataSource::AppleMusic,
        });

        assert_eq!(meta.title, "Chulo pt.2 (feat. Tokischa & Young Miko)");
        assert_eq!(meta.artist, "Bad Gyal");
        assert_eq!(meta.album_artist, "Bad Gyal");
        assert_eq!(meta.album, "La Joia");
    }

    #[test]
    fn handles_single_and_compilation_and_multidisc_rules() {
        let single = normalize_track_metadata(RawMetadataInput {
            title: "Bling-Bang-Bang-Born".to_string(),
            artist: "Creepy Nuts".to_string(),
            collection_artist: None,
            album: Some("Bling-Bang-Bang-Born - Single".to_string()),
            track_number: Some(1),
            track_total: Some(1),
            disc_number: None,
            disc_total: None,
            year: Some(2024),
            genre: Some("Hip-Hop/Rap".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: MetadataSource::AppleMusic,
        });
        assert_eq!(single.album, "Bling-Bang-Bang-Born");

        let ost = normalize_track_metadata(RawMetadataInput {
            title: "Sunflower".to_string(),
            artist: "Post Malone & Swae Lee".to_string(),
            collection_artist: Some("Various Artists".to_string()),
            album: Some("Spider-Man: Into the Spider-Verse (Original Motion Picture Soundtrack)".to_string()),
            track_number: Some(2),
            track_total: Some(13),
            disc_number: Some(2),
            disc_total: Some(2),
            year: Some(2018),
            genre: Some("Soundtrack".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: MetadataSource::AppleMusic,
        });
        assert!(ost.is_compilation);
        assert_eq!(ost.album_artist, "Various Artists");
        assert_eq!(ost.artist, "Post Malone & Swae Lee");
        assert_eq!(ost.disc_number, Some(2));
        assert_eq!(ost.disc_total, Some(2));
    }

    #[test]
    fn builds_clean_search_query_from_noisy_filename() {
        let parsed = build_search_query(
            "01 - Mariah Carey - Reflections (Official Audio) [320kbps].mp3",
            None,
            None,
        );
        assert_eq!(parsed.artist_hint.as_deref(), Some("Mariah Carey"));
        assert_eq!(parsed.title_hint, "Reflections");
        assert_eq!(parsed.search_term, "Mariah Carey Reflections");
    }
}

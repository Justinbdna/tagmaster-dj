//! Règles métier strictes de tagging (SPECIFICATIONS.md §4.1 & §4.3) :
//! - Featurings normalisés dans le titre (`Chulo pt.2 (feat. Tokischa & Young Miko)`)
//!   et `Album Artist` verrouillé sur l'artiste principal unique.
//! - Singles : `Album` reprend le nom du single.
//! - Compilations / BO : `Album Artist` = `Various Artists`, vrai interprète conservé dans `Artist`.
//! - Multi-disques : `Disc Number` et `Disc Total` conservés (`1/2`, `2/2`).
//! - Mots-clés intouchables : empêcher Apple Music d'écraser une version alternative.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::models::{ExistingTrackTags, MetadataSource, TrackMetadata};

pub const PROTECTED_KEYWORDS: &[&str] = &[
    "live",
    "acoustic",
    "sped up",
    "slowed",
    "reverb",
    "nightcore",
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

pub const GENERIC_FOLDER_NAMES: &[&str] = &[
    "downloads",
    "telechargements",
    "music",
    "musique",
    "my music",
    "ma musique",
    "desktop",
    "bureau",
    "documents",
    "mes documents",
    "users",
    "utilisateurs",
    "home",
    "volumes",
    "tmp",
    "temp",
    "var",
    "private",
    "library",
    "bibliotheque",
    "applications",
    "public",
    "shared",
    "partage",
    "icloud drive",
    "tagmaster dj export",
    "tagmaster dj",
    "export",
    "exports",
    "import",
    "imports",
    "inbox",
    "unsorted",
    "misc",
    "divers",
    "audio",
    "songs",
    "tracks",
    "mp3",
    "flac",
    "wav",
    "new folder",
    "nouveau dossier",
    "untitled folder",
    "dossier sans titre",
    "media",
    "itunes",
    "apple music",
];

const GENERIC_ARTIST_PLACEHOLDERS: &[&str] = &[
    "various artists",
    "multi interpretes",
    "compilation",
    "unknown",
    "unknown artist",
    "artiste inconnu",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrackQuery {
    pub artist_hint: Option<String>,
    pub title_hint: String,
    pub album_hint: Option<String>,
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
        Regex::new(
            r"(?i)^\s*(?:(?:track|piste)\s+)?(?:[\(\[]\s*(?:\d{1,2}\s*[-.]\s*)?\d{1,3}\s*[\)\]]\s*(?:[-._–—]\s*|\s+)|(?:\d{1,2}\s*[-.]\s*)?\d{1,3}(?:\s*[-._–—)\]]\s*|\s+))",
        )
        .expect("valid track prefix regex")
    })
}

fn noise_brackets_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)[\(\[](?:official\s*(?:music\s*)?(?:video|audio|lyric\s*video|visualizer)|clip\s*officiel|audio\s*officiel|vid[ée]o\s*officielle|lyrics?|paroles|audio|(?:full\s*)?hd|hq|4k|320\s*kbps|explicit|clean|music\s*video|visualizer|exclus?|unreleased|tiktok|prod\.?(?:\s+by)?\s+[^\)\]]+)[\)\]]",
        )
        .expect("valid noise regex")
    })
}

fn leading_unreleased_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*[!¡]*\s*(?:unreleased|exclus?)\s*(?:[-–—:]\s*|\s+)")
            .expect("valid leading unreleased regex")
    })
}

fn disc_folder_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(?:cd|disc|disk|disque)\s*\d+$").expect("valid disc regex"))
}

fn title_feat_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:\s*[\(\[]\s*|\s+)(?:feat\.?|ft\.?|featuring)\s+([^\)\]]+)[\)\]]?")
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

/// Harmonise apostrophes typographiques et espaces insécables.
pub fn normalize_apostrophes(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '’' | '‘' | 'ʼ' | '´' | '`' => '\'',
            '\u{00A0}' => ' ',
            other => other,
        })
        .collect()
}

fn fold_latin_chars(input: &str) -> String {
    let normalized = normalize_apostrophes(input).to_lowercase();
    let mut folded = String::with_capacity(normalized.len());
    for ch in normalized.chars() {
        match ch {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => folded.push('a'),
            'ç' => folded.push('c'),
            'è' | 'é' | 'ê' | 'ë' => folded.push('e'),
            'ì' | 'í' | 'î' | 'ï' => folded.push('i'),
            'ñ' => folded.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => folded.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => folded.push('u'),
            'ý' | 'ÿ' => folded.push('y'),
            'œ' => folded.push_str("oe"),
            'æ' => folded.push_str("ae"),
            'ß' => folded.push_str("ss"),
            other => folded.push(other),
        }
    }
    folded
}

/// Normalise une chaîne pour la comparaison floue (casse, accents, apostrophes, espaces).
pub fn normalize_for_match(input: &str) -> String {
    fold_latin_chars(input)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

const MUSICAL_NUMBER_QUALIFIERS: &[&str] = &[
    "pt", "part", "vol", "v", "ver", "version", "no", "n", "cd", "disc", "disk", "act", "chapter",
    "remix", "mix", "edit", "ep",
];

fn copy_word_suffix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\s*(?:[-–—]\s*)?(?:copy|copie)(?:\s*\d{1,2})?\s*$")
            .expect("valid copy suffix regex")
    })
}

fn paren_copy_suffix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s*\(\d{1,2}\)\s*$").expect("valid paren copy suffix regex"))
}

fn double_underscore_copy_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"__+\d{1,2}\s*$").expect("valid double underscore copy regex"))
}

fn single_underscore_copy_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"_\d{1,2}\s*$").expect("valid single underscore copy regex"))
}

/// Supprime les suffixes techniques de doublons en fin de libellé (`__2`, `_1`, `(1)`,
/// `(2)`, ` - Copy`, ` copie`) tout en préservant les mentions musicales légitimes (`pt. 2`,
/// `Part 2`, `Vol. 2`, `v2`, `Remix 2`, `Chulo_pt_2`).
pub fn strip_duplicate_copy_suffix(input: &str) -> String {
    let mut current = input.trim_end().to_string();

    loop {
        let mut next = current.clone();

        if let Some(m) = copy_word_suffix_re().find(&next) {
            let candidate = next[..m.start()].trim_end();
            if !candidate.is_empty() {
                next = candidate.to_string();
            }
        } else if let Some(m) = paren_copy_suffix_re().find(&next) {
            let candidate = next[..m.start()].trim_end();
            if !candidate.is_empty() {
                next = candidate.to_string();
            }
        } else if let Some(m) = double_underscore_copy_re().find(&next) {
            let candidate = next[..m.start()].trim_end();
            if !candidate.is_empty() {
                next = candidate.to_string();
            }
        } else if let Some(m) = single_underscore_copy_re().find(&next) {
            let prefix = next[..m.start()].trim_end();
            let prev_word = prefix
                .rsplit(|c: char| c.is_whitespace() || matches!(c, '_' | '-' | '/' | '(' | '['))
                .next()
                .unwrap_or("")
                .trim_end_matches('.')
                .to_ascii_lowercase();

            if !prefix.is_empty() && !MUSICAL_NUMBER_QUALIFIERS.contains(&prev_word.as_str()) {
                next = prefix.to_string();
            }
        }

        if next == current {
            break;
        }
        current = next;
    }

    current
}

/// Nettoie un libellé de piste (titre ou nom sans extension) en supprimant numérotation,
/// suffixes de doublons (`__2`, `(1)`, ` - Copy`) et mentions parasites (`Official Audio`, `HD`, `Lyrics`)
/// sans toucher aux mots-clés protégés ni aux mentions musicales (`pt. 2`, `Vol. 2`, `v2`).
pub fn clean_track_label(raw: &str) -> String {
    let normalized = normalize_apostrophes(raw);
    let without_noise = noise_brackets_re().replace_all(&normalized, " ");
    let without_copy = strip_duplicate_copy_suffix(&without_noise);
    let without_underscores = without_copy.replace('_', " ");
    let without_unreleased = if let Some(m) = leading_unreleased_re().find(&without_underscores)
        && !without_underscores[m.end()..].trim().is_empty()
    {
        &without_underscores[m.end()..]
    } else {
        without_underscores.as_str()
    };
    let without_prefix = leading_track_num_re().replace(without_unreleased, "");
    let cleaned = collapse_spaces(&without_prefix);
    if cleaned.is_empty() {
        collapse_spaces(&without_underscores)
    } else {
        cleaned
    }
}

/// Nettoie un nom de fichier audio après retrait de son extension.
pub fn clean_filename_stem(file_name: &str) -> String {
    let stem = file_name
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(file_name);
    clean_track_label(stem)
}

pub fn split_artist_title(input: &str) -> Option<(String, String)> {
    for sep in [" - ", " – ", " — "] {
        if let Some((left, right)) = input.split_once(sep) {
            let artist = collapse_spaces(left);
            let title = collapse_spaces(right);
            if !artist.is_empty() && !title.is_empty() {
                return Some((artist, title));
            }
        }
    }
    None
}

fn is_generic_artist_placeholder(value: &str) -> bool {
    let norm = normalize_for_match(value);
    norm.is_empty() || GENERIC_ARTIST_PLACEHOLDERS.contains(&norm.as_str())
}

/// Extrait des indices `(artiste, album)` depuis les dossiers parents en ignorant
/// les dossiers génériques (`Downloads`, `Téléchargements`, `Music`, `Desktop`, etc.).
pub fn extract_folder_hints(source_path: &Path) -> (Option<String>, Option<String>) {
    let mut usable: Vec<String> = Vec::with_capacity(2);
    let mut current = source_path.parent();

    for _ in 0..4 {
        let Some(dir) = current else {
            break;
        };
        let Some(raw_name) = dir.file_name().and_then(|n| n.to_str()) else {
            break;
        };

        let trimmed = raw_name.trim();
        if disc_folder_re().is_match(trimmed) {
            current = dir.parent();
            continue;
        }

        let parent_is_user_root = dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|p| matches!(normalize_for_match(p).as_str(), "users" | "utilisateurs" | "home"))
            .unwrap_or(false);

        let norm = normalize_for_match(trimmed);
        if norm.is_empty() || parent_is_user_root || GENERIC_FOLDER_NAMES.contains(&norm.as_str()) {
            break;
        }

        let cleaned_dir = collapse_spaces(&normalize_apostrophes(&trimmed.replace('_', " ")));
        if !cleaned_dir.is_empty() {
            usable.push(cleaned_dir);
            if usable.len() == 2 {
                break;
            }
        }

        current = dir.parent();
    }

    match usable.as_slice() {
        [closest, second, ..] => (Some(second.clone()), Some(closest.clone())),
        [single] => {
            if let Some((left, right)) = split_artist_title(single) {
                (Some(left), Some(right))
            } else {
                (Some(single.clone()), None)
            }
        }
        [] => (None, None),
    }
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

/// Construit une requête de recherche enrichie à partir du nom de fichier, des tags
/// existants (`artist`, `album_artist`, `album`, `title`) et des dossiers parents.
pub fn build_search_query(source_path: &Path, existing: &ExistingTrackTags) -> ParsedTrackQuery {
    let file_name = source_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source_path.to_string_lossy().into_owned());

    let cleaned_filename = clean_filename_stem(&file_name);
    let (filename_artist, filename_title) = match split_artist_title(&cleaned_filename) {
        Some((artist, title)) => (Some(artist), Some(title)),
        None => (None, None),
    };

    let clean_existing_artist = existing
        .artist
        .as_deref()
        .map(|s| collapse_spaces(&normalize_apostrophes(s)))
        .filter(|s| !s.is_empty() && !looks_like_dirty_tag(s) && !is_generic_artist_placeholder(s));

    let clean_existing_album_artist = existing
        .album_artist
        .as_deref()
        .map(|s| collapse_spaces(&normalize_apostrophes(s)))
        .filter(|s| !s.is_empty() && !looks_like_dirty_tag(s) && !is_generic_artist_placeholder(s));

    let clean_existing_album = existing
        .album
        .as_deref()
        .map(|s| collapse_spaces(&normalize_apostrophes(s)))
        .filter(|s| !s.is_empty() && !looks_like_dirty_tag(s));

    let clean_existing_title = existing
        .title
        .as_deref()
        .filter(|s| !looks_like_dirty_tag(s))
        .map(clean_track_label)
        .filter(|s| !s.is_empty());

    let (folder_artist, folder_album) = extract_folder_hints(source_path);

    let artist_raw = filename_artist
        .clone()
        .or(clean_existing_artist)
        .or(clean_existing_album_artist)
        .or(folder_artist);

    let title_hint = if filename_artist.is_some() {
        filename_title.unwrap_or(cleaned_filename)
    } else {
        clean_existing_title.unwrap_or(cleaned_filename)
    };

    let album_hint = clean_existing_album.or(folder_album);

    let artist_hint = artist_raw
        .as_deref()
        .map(extract_primary_artist)
        .filter(|s| !s.is_empty());

    let base_title = {
        let stripped = strip_feat_from_title(&title_hint);
        if stripped.is_empty() {
            title_hint.clone()
        } else {
            stripped
        }
    };

    let search_term = match &artist_hint {
        Some(primary) => collapse_spaces(&format!("{primary} {base_title}")),
        None => base_title,
    };

    ParsedTrackQuery {
        artist_hint,
        title_hint,
        album_hint,
        search_term,
    }
}

pub fn strip_feat_from_title(title: &str) -> String {
    collapse_spaces(&title_feat_re().replace_all(title, ""))
}

/// Normalise les principaux genres musicaux (notamment renvoyés en français par Apple Music FR)
/// vers leurs équivalents anglais unifiés afin d'éviter l'éclatement des bibliothèques DJ.
pub fn normalize_genre_to_english(raw: &str) -> String {
    let collapsed = collapse_spaces(&normalize_apostrophes(raw));
    if collapsed.is_empty() {
        return String::new();
    }

    let key = collapse_spaces(&fold_latin_chars(&collapsed));
    match key.as_str() {
        "hip-hop/rap"
        | "hip hop/rap"
        | "hip-hop / rap"
        | "hip hop / rap"
        | "rap/hip-hop"
        | "rap / hip-hop"
        | "rap"
        | "rap francais"
        | "hip-hop francais"
        | "hip hop francais" => "Hip-Hop/Rap".to_string(),
        "variete francaise"
        | "chanson francaise"
        | "chanson en francais"
        | "chanson"
        | "pop francaise"
        | "french pop" => "French Pop".to_string(),
        "pop" | "musique pop" => "Pop".to_string(),
        "r&b/soul"
        | "r&b / soul"
        | "soul/r&b"
        | "soul / r&b"
        | "r&b contemporain"
        | "r&b" => "R&B/Soul".to_string(),
        "electronique"
        | "musique electronique"
        | "electro"
        | "electronic" => "Electronic".to_string(),
        "dance" | "musique dance" => "Dance".to_string(),
        "musique latine" | "musiques latines" | "latino" | "latin" => "Latin".to_string(),
        "afro pop" | "afro-pop" | "afropop" => "Afro-Pop".to_string(),
        "afrobeat" | "afrobeats" => "Afrobeats".to_string(),
        "alternatif" | "musique alternative" | "alternative" => "Alternative".to_string(),
        "rock" | "rock francais" => "Rock".to_string(),
        "metal" | "heavy metal" => "Metal".to_string(),
        "musique classique" | "classique" | "classical" => "Classical".to_string(),
        "bande originale"
        | "bandes originales"
        | "musique de film"
        | "musiques de films"
        | "soundtrack" => "Soundtrack".to_string(),
        "musiques du monde" | "musique du monde" | "worldwide" => "Worldwide".to_string(),
        "musique pour enfants" | "chanson pour enfants" | "enfants" => {
            "Children's Music".to_string()
        }
        "musique de noel" | "noel" | "holiday" => "Holiday".to_string(),
        "comedie" | "humour" | "comedy" => "Comedy".to_string(),
        "jazz" => "Jazz".to_string(),
        "reggae" => "Reggae".to_string(),
        "blues" => "Blues".to_string(),
        "country" => "Country".to_string(),
        _ => collapsed,
    }
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
        .map(|g| normalize_genre_to_english(&g))
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
        lyrics: None,
        synced_lyrics: None,
        bpm: None,
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
        assert_eq!(
            detect_protected_keyword("CLM-RST - still love you (Slowed).mp3"),
            Some("slowed")
        );
        assert_eq!(
            detect_protected_keyword("Magdalena Bay - Killshot (Slowed + Reverb).mp3"),
            Some("slowed")
        );
        assert_eq!(
            detect_protected_keyword("Be Like A Woman (Slowed Down Version).mp3"),
            Some("slowed")
        );
        assert_eq!(
            detect_protected_keyword("Song (Nightcore).mp3"),
            Some("nightcore")
        );
        // Faux positifs à éviter
        assert_eq!(detect_protected_keyword("Olivia Rodrigo - Drivers License.mp3"), None);
        assert_eq!(detect_protected_keyword("Adele - Deliverance.mp3"), None);
        assert_eq!(detect_protected_keyword("Niall Horan - Slow Hands.mp3"), None);
    }

    #[test]
    fn cleans_unreleased_and_exclus_prefixes_before_splitting_artist_title() {
        let parsed = build_search_query(
            Path::new("/Users/jujutravail/Music/_! Unreleased - Kim-Petras - Deeper.mp3"),
            &ExistingTrackTags::default(),
        );
        assert_eq!(parsed.artist_hint.as_deref(), Some("Kim-Petras"));
        assert_eq!(parsed.title_hint, "Deeper");
        assert_eq!(parsed.search_term, "Kim-Petras Deeper");

        assert_eq!(
            clean_filename_stem("[EXCLUS] La Fève - FINIS-LES REMIX [Prod. @B.June].mp3"),
            "La Fève - FINIS-LES REMIX"
        );
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

        // Ne jamais interpréter la fin d'un mot en "-ft" (Daft, Left, Soft) comme "ft."
        let daft = normalize_track_metadata(RawMetadataInput {
            title: "Get Lucky (Daft Punk Remix)".to_string(),
            artist: "Daft Punk".to_string(),
            collection_artist: None,
            album: None,
            track_number: None,
            track_total: None,
            disc_number: None,
            disc_total: None,
            year: Some(2013),
            genre: None,
            artwork_url: None,
            is_compilation: false,
            source: MetadataSource::YtDlp,
        });
        assert_eq!(daft.title, "Get Lucky (Daft Punk Remix)");
        assert_eq!(strip_feat_from_title("Left & Right"), "Left & Right");
        assert_eq!(strip_feat_from_title("Soft Touch"), "Soft Touch");
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
            Path::new("01 - Mariah Carey - Reflections (Official Audio) [320kbps].mp3"),
            &ExistingTrackTags::default(),
        );
        assert_eq!(parsed.artist_hint.as_deref(), Some("Mariah Carey"));
        assert_eq!(parsed.title_hint, "Reflections");
        assert_eq!(parsed.search_term, "Mariah Carey Reflections");
    }

    #[test]
    fn strips_track_number_prefixes_while_preserving_protected_keywords_and_years() {
        for sample in [
            "05 T mimi.mp3",
            "05 - T mimi.mp3",
            "05. T mimi.mp3",
            "1-05 T mimi.mp3",
            "(05) T mimi.mp3",
            "[05] - T mimi (Official Audio) [HD].mp3",
        ] {
            assert_eq!(clean_filename_stem(sample), "T mimi", "failed on {sample}");
        }

        assert_eq!(clean_filename_stem("1999 - Prince.mp3"), "1999 - Prince");
        assert_eq!(clean_filename_stem("24K Magic.mp3"), "24K Magic");
        assert_eq!(clean_filename_stem("05 T mimi (Remix).mp3"), "T mimi (Remix)");
        assert_eq!(clean_filename_stem("05 Song (Sped Up).mp3"), "Song (Sped Up)");
        assert_eq!(clean_filename_stem("01 Track (Live).flac"), "Track (Live)");
    }

    #[test]
    fn enriches_query_with_existing_artist_and_album_artist_tags() {
        let with_artist_only = build_search_query(
            Path::new("/Users/jujutravail/Downloads/05 T mimi.mp3"),
            &ExistingTrackTags {
                artist: Some("Shay".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(with_artist_only.artist_hint.as_deref(), Some("Shay"));
        assert_eq!(with_artist_only.title_hint, "T mimi");
        assert_eq!(with_artist_only.search_term, "Shay T mimi");

        let with_album_artist_only = build_search_query(
            Path::new("/Users/jujutravail/Downloads/05 T mimi.mp3"),
            &ExistingTrackTags {
                album_artist: Some("Shay".to_string()),
                album: Some("Pourvu qu'il pleuve".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(with_album_artist_only.artist_hint.as_deref(), Some("Shay"));
        assert_eq!(
            with_album_artist_only.album_hint.as_deref(),
            Some("Pourvu qu'il pleuve")
        );
        assert_eq!(with_album_artist_only.title_hint, "T mimi");
        assert_eq!(with_album_artist_only.search_term, "Shay T mimi");
    }

    #[test]
    fn enriches_query_from_parent_folders_and_ignores_generic_folders() {
        let from_folders = build_search_query(
            Path::new("/Users/jujutravail/Music/Shay/Pourvu qu'il pleuve/05 T mimi.mp3"),
            &ExistingTrackTags::default(),
        );
        assert_eq!(from_folders.artist_hint.as_deref(), Some("Shay"));
        assert_eq!(
            from_folders.album_hint.as_deref(),
            Some("Pourvu qu'il pleuve")
        );
        assert_eq!(from_folders.title_hint, "T mimi");
        assert_eq!(from_folders.search_term, "Shay T mimi");

        for generic in [
            "/Users/jujutravail/Downloads/05 T mimi.mp3",
            "/Users/jujutravail/Téléchargements/05 T mimi.mp3",
            "/Users/jujutravail/Desktop/05 T mimi.mp3",
            "/Users/jujutravail/Bureau/05 T mimi.mp3",
        ] {
            let parsed = build_search_query(Path::new(generic), &ExistingTrackTags::default());
            assert_eq!(parsed.artist_hint, None, "generic folder leaked on {generic}");
            assert_eq!(parsed.title_hint, "T mimi");
            assert_eq!(parsed.search_term, "T mimi");
        }
    }

    #[test]
    fn normalizes_accents_apostrophes_and_genres() {
        assert_eq!(normalize_for_match("Más Cara"), normalize_for_match("Mas Cara"));
        assert_eq!(normalize_for_match("T’mimi"), "t mimi");
        assert_eq!(normalize_for_match("T'mimi"), "t mimi");
        assert_eq!(normalize_for_match("T   mimi"), "t mimi");

        assert_eq!(normalize_genre_to_english("Hip-hop/Rap"), "Hip-Hop/Rap");
        assert_eq!(normalize_genre_to_english("Variété française"), "French Pop");
        assert_eq!(normalize_genre_to_english("Électronique"), "Electronic");
        assert_eq!(normalize_genre_to_english("Musique latine"), "Latin");
        assert_eq!(normalize_genre_to_english("R&B / Soul"), "R&B/Soul");
        assert_eq!(normalize_genre_to_english("Afro\u{00a0}pop"), "Afro-Pop");
        assert_eq!(normalize_genre_to_english("Urbano latino"), "Urbano latino");
    }

    #[test]
    fn strips_duplicate_copy_suffixes_while_preserving_musical_numbers() {
        let nuevayol = build_search_query(
            Path::new("/Users/jujutravail/Downloads/Bad Bunny - NUEVAYoL__2.mp3"),
            &ExistingTrackTags::default(),
        );
        assert_eq!(nuevayol.artist_hint.as_deref(), Some("Bad Bunny"));
        assert_eq!(nuevayol.title_hint, "NUEVAYoL");
        assert_eq!(nuevayol.search_term, "Bad Bunny NUEVAYoL");

        for sample in [
            "Bad Bunny - NUEVAYoL__3.mp3",
            "Bad Bunny - NUEVAYoL_1.mp3",
            "Bad Bunny - NUEVAYoL_2.mp3",
            "Bad Bunny - NUEVAYoL (1).mp3",
            "Bad Bunny - NUEVAYoL (2).mp3",
            "Bad Bunny - NUEVAYoL - Copy.mp3",
            "Bad Bunny - NUEVAYoL copie.mp3",
            "Bad Bunny - NUEVAYoL__2 (Official Audio).mp3",
        ] {
            assert_eq!(
                clean_filename_stem(sample),
                "Bad Bunny - NUEVAYoL",
                "failed on {sample}"
            );
        }

        assert_eq!(clean_filename_stem("Bad Gyal - Chulo pt.2.mp3"), "Bad Gyal - Chulo pt.2");
        assert_eq!(clean_filename_stem("Bad Gyal - Chulo pt. 2.mp3"), "Bad Gyal - Chulo pt. 2");
        assert_eq!(clean_filename_stem("Artist - Song Part 2.mp3"), "Artist - Song Part 2");
        assert_eq!(clean_filename_stem("Artist - Mix Vol. 2.mp3"), "Artist - Mix Vol. 2");
        assert_eq!(clean_filename_stem("Artist - Track v2.mp3"), "Artist - Track v2");
        assert_eq!(
            clean_filename_stem("Artist - Track (Remix 2).mp3"),
            "Artist - Track (Remix 2)"
        );
        assert_eq!(
            clean_filename_stem("Bad_Gyal_-_Chulo_pt_2.mp3"),
            "Bad Gyal - Chulo pt 2"
        );
    }
}

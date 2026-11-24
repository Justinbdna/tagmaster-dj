//! Sécurité Windows : protection contre la limite de 256 caractères des chemins physiques,
//! nettoyage des caractères interdits (`< > : " / \ | ? *`), noms réservés Windows,
//! et routage Plat / Dynamique (`SPECIFICATIONS.md` §5.2).

use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{ExportMode, TrackMetadata};

pub const WINDOWS_MAX_PATH_SAFE: usize = 248;
const MAX_FOLDER_SEGMENT_CHARS: usize = 64;

const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn is_windows_reserved_name(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    WINDOWS_RESERVED_NAMES
        .iter()
        .any(|reserved| base.eq_ignore_ascii_case(reserved))
}

/// Nettoie un segment de chemin (dossier ou nom de fichier) pour Windows et macOS.
pub fn sanitize_filename_segment(segment: &str) -> String {
    let replaced: String = segment
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            _ => c,
        })
        .collect();

    let collapsed = replaced
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let trimmed = collapsed.trim().trim_end_matches('.').trim().to_string();

    if trimmed.is_empty() {
        return "Inconnu".to_string();
    }

    if is_windows_reserved_name(&trimmed) {
        format!("_{trimmed}")
    } else {
        trimmed
    }
}

fn truncate_folder_segment(segment: &str, max_chars: usize) -> String {
    let cleaned = sanitize_filename_segment(segment);
    if cleaned.chars().count() <= max_chars {
        return cleaned;
    }

    let truncated: String = cleaned.chars().take(max_chars).collect();
    let safe = truncated.trim_end_matches(['.', ' ']).to_string();
    if safe.is_empty() {
        "Inconnu".to_string()
    } else {
        safe
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    input
        .chars()
        .take(max_chars)
        .collect::<String>()
        .trim_end_matches(['.', ' '])
        .to_string()
}

fn normalized_extension(source_extension: &str) -> String {
    let ext = source_extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if ext.is_empty() {
        "mp3".to_string()
    } else {
        ext
    }
}

/// Construit le chemin cible Plat ou Dynamique en garantissant que sa longueur totale
/// ne dépasse pas `WINDOWS_MAX_PATH_SAFE` (248 caractères), avec troncature intelligente `...`
/// tout en conservant les métadonnées internes intactes.
pub fn build_export_path(
    export_root: &Path,
    mode: ExportMode,
    metadata: &TrackMetadata,
    source_extension: &str,
) -> PathBuf {
    let ext = normalized_extension(source_extension);
    let dot_ext = format!(".{ext}");
    let artist_seg = sanitize_filename_segment(&metadata.artist);
    let title_seg = sanitize_filename_segment(&metadata.title);
    let stem = format!("{artist_seg} - {title_seg}");

    let parent_dir = match mode {
        ExportMode::Flat => export_root.to_path_buf(),
        ExportMode::Dynamic => {
            let primary_artist = if metadata.album_artist.trim().is_empty() {
                &metadata.artist
            } else {
                &metadata.album_artist
            };
            let folder_artist = truncate_folder_segment(primary_artist, MAX_FOLDER_SEGMENT_CHARS);
            let folder_album = truncate_folder_segment(&metadata.album, MAX_FOLDER_SEGMENT_CHARS);
            export_root.join(folder_artist).join(folder_album)
        }
    };

    let desired_file_name = format!("{stem}{dot_ext}");
    let candidate = parent_dir.join(&desired_file_name);

    // Mesurer sur le chemin non-UNC pour refléter la longueur réelle Windows
    let candidate_len = candidate.to_string_lossy().chars().count();
    if candidate_len <= WINDOWS_MAX_PATH_SAFE {
        return candidate;
    }

    let parent_len = parent_dir.to_string_lossy().chars().count();
    let overhead = parent_len + 1 /* séparateur */ + 3 /* "..." */ + dot_ext.chars().count();
    let stem_budget = WINDOWS_MAX_PATH_SAFE.saturating_sub(overhead).max(8);

    let short_stem = truncate_chars(&stem, stem_budget);
    let final_stem = if short_stem.is_empty() {
        "track".to_string()
    } else {
        short_stem
    };

    parent_dir.join(format!("{final_stem}...{dot_ext}"))
}

fn paths_refer_to_same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// Évite tout écrasement d'un fichier déjà exporté ou du fichier source en ajoutant
/// un suffixe ` (1)`, ` (2)`, etc. tout en respectant la limite de longueur Windows.
pub fn resolve_non_colliding_path(desired_path: &Path, source_path: &Path) -> PathBuf {
    if !desired_path.exists() && !paths_refer_to_same_file(desired_path, source_path) {
        return desired_path.to_path_buf();
    }

    let parent = desired_path.parent().unwrap_or_else(|| Path::new("."));
    let ext = desired_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let stem = desired_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");

    let parent_len = parent.to_string_lossy().chars().count();
    let ext_len = ext.chars().count();

    for counter in 1..10_000 {
        let suffix = format!(" ({counter})");
        let overhead = parent_len + 1 + suffix.chars().count() + ext_len;
        let max_stem = 255usize.saturating_sub(overhead).max(4);
        let safe_stem = if stem.chars().count() > max_stem {
            stem.chars().take(max_stem).collect::<String>()
        } else {
            stem.to_string()
        };

        let candidate = parent.join(format!("{safe_stem}{suffix}{ext}"));
        if !candidate.exists() && !paths_refer_to_same_file(&candidate, source_path) {
            return candidate;
        }
    }

    desired_path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MetadataSource;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_meta(artist: &str, album_artist: &str, album: &str, title: &str) -> TrackMetadata {
        TrackMetadata {
            title: title.to_string(),
            artist: artist.to_string(),
            album_artist: album_artist.to_string(),
            album: album.to_string(),
            track_number: Some(1),
            track_total: Some(10),
            disc_number: Some(1),
            disc_total: Some(1),
            year: Some(2024),
            genre: Some("Pop".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: Some(MetadataSource::AppleMusic),
        }
    }

    #[test]
    fn sanitizes_windows_forbidden_chars_reserved_names_and_trailing_dots() {
        assert_eq!(
            sanitize_filename_segment(r#"AC/DC: What? *Live* "2024"..."#),
            "AC_DC_ What_ _Live_ _2024_"
        );
        assert_eq!(sanitize_filename_segment("CON"), "_CON");
        assert_eq!(sanitize_filename_segment("nul.mp3"), "_nul.mp3");
        assert_eq!(sanitize_filename_segment("   ...   "), "Inconnu");
    }

    #[test]
    fn builds_flat_and_dynamic_paths_correctly() {
        let root = Path::new(r"C:\Users\Justin\Documents\Tagmaster_DJ_Export");
        let meta = sample_meta(
            "Bad Gyal",
            "Bad Gyal",
            "La Joia",
            "Chulo pt.2 (feat. Tokischa & Young Miko)",
        );

        let flat = build_export_path(root, ExportMode::Flat, &meta, "MP3");
        assert_eq!(
            flat,
            root.join("Bad Gyal - Chulo pt.2 (feat. Tokischa & Young Miko).mp3")
        );

        let dynamic = build_export_path(root, ExportMode::Dynamic, &meta, "flac");
        assert_eq!(
            dynamic,
            root.join("Bad Gyal")
                .join("La Joia")
                .join("Bad Gyal - Chulo pt.2 (feat. Tokischa & Young Miko).flac")
        );

        let comp_meta = sample_meta(
            "Post Malone & Swae Lee",
            "Various Artists",
            "Spider-Man: OST",
            "Sunflower",
        );
        let comp_dynamic = build_export_path(root, ExportMode::Dynamic, &comp_meta, "mp3");
        assert_eq!(
            comp_dynamic,
            root.join("Various Artists")
                .join("Spider-Man_ OST")
                .join("Post Malone & Swae Lee - Sunflower.mp3")
        );
    }

    #[test]
    fn truncates_long_paths_under_windows_limit_with_ellipsis() {
        let root = Path::new(r"C:\Users\Justin\Documents\Tagmaster_DJ_Export");
        let very_long_title = "Very Long Track Title ".repeat(16);
        let meta = sample_meta(
            "Artist With Long Name",
            "Artist With Long Name",
            "Album With A Moderately Long Title For Testing Path Boundaries",
            &very_long_title,
        );

        let result = build_export_path(root, ExportMode::Dynamic, &meta, "mp3");
        let total_chars = result.to_string_lossy().chars().count();
        let file_name = result.file_name().unwrap().to_string_lossy();

        assert!(
            total_chars <= WINDOWS_MAX_PATH_SAFE,
            "path length {total_chars} exceeded {WINDOWS_MAX_PATH_SAFE}"
        );
        assert!(file_name.ends_with("....mp3"));
    }

    #[test]
    fn resolves_non_colliding_path_when_target_exists() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tagmaster_dj_collision_test_{nanos}"));
        fs::create_dir_all(&dir).unwrap();

        let existing = dir.join("Daft Punk - Get Lucky.mp3");
        let source = dir.join("source.mp3");
        fs::write(&existing, b"already-exported").unwrap();
        fs::write(&source, b"source").unwrap();

        let next = resolve_non_colliding_path(&existing, &source);
        assert_eq!(next, dir.join("Daft Punk - Get Lucky (1).mp3"));

        let _ = fs::remove_dir_all(&dir);
    }
}

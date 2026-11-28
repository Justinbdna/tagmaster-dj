//! Sanitizer de caractères spéciaux, emojis et compatibilité FAT32 / ASCII
//! pour les profils matériels iPod et PSP (SPECIFICATIONS.md §4.3 · Issue #16).

use crate::models::{HardwareProfile, TrackMetadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizeLevel {
    /// UTF-8 complet sans restriction (profil Standard).
    Standard,
    /// Compatible FAT32 / iPod : supprime emojis et caractères de contrôle, conserve les accents latins.
    Fat32,
    /// Strict ASCII / PSP (Memory Stick Pro Duo & XMB) : ASCII pur imprimable uniquement.
    StrictAscii,
}

impl SanitizeLevel {
    pub fn for_profile(profile: &HardwareProfile) -> Self {
        match profile {
            HardwareProfile::Standard => Self::Standard,
            HardwareProfile::Ipod => Self::Fat32,
            HardwareProfile::Psp => Self::StrictAscii,
        }
    }
}

fn fold_to_ascii_str(ch: char) -> Option<&'static str> {
    match ch {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => Some("A"),
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => Some("a"),
        'Ç' => Some("C"),
        'ç' => Some("c"),
        'È' | 'É' | 'Ê' | 'Ë' => Some("E"),
        'è' | 'é' | 'ê' | 'ë' => Some("e"),
        'Ì' | 'Í' | 'Î' | 'Ï' => Some("I"),
        'ì' | 'í' | 'î' | 'ï' => Some("i"),
        'Ñ' => Some("N"),
        'ñ' => Some("n"),
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => Some("O"),
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => Some("o"),
        'Ù' | 'Ú' | 'Û' | 'Ü' => Some("U"),
        'ù' | 'ú' | 'û' | 'ü' => Some("u"),
        'Ý' | 'Ÿ' => Some("Y"),
        'ý' | 'ÿ' => Some("y"),
        'Œ' => Some("OE"),
        'œ' => Some("oe"),
        'Æ' => Some("AE"),
        'æ' => Some("ae"),
        'ß' => Some("ss"),
        '’' | '‘' | 'ʼ' | '´' | '`' => Some("'"),
        '“' | '”' | '«' | '»' => Some("\""),
        '–' | '—' | '−' => Some("-"),
        '…' => Some("..."),
        '\u{00A0}' => Some(" "),
        _ => None,
    }
}

fn is_emoji_or_symbol_unsafe_for_retro(ch: char) -> bool {
    let code = ch as u32;
    matches!(
        code,
        0x200B..=0x200F // zero-width / bidi
        | 0x2028..=0x202F
        | 0x2190..=0x2BFF // flèches, symboles techniques, dingbats, étoiles
        | 0xFE00..=0xFE0F // variation selectors
        | 0x1F000..=0x1FFFF // emojis / pictographes
    )
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Nettoie une chaîne de métadonnées selon le niveau matériel demandé.
pub fn sanitize_text(input: &str, level: SanitizeLevel) -> String {
    match level {
        SanitizeLevel::Standard => input.to_string(),
        SanitizeLevel::Fat32 => {
            let mut out = String::with_capacity(input.len());
            for ch in input.chars() {
                if ch.is_control() || is_emoji_or_symbol_unsafe_for_retro(ch) {
                    continue;
                }
                match ch {
                    '’' | '‘' | 'ʼ' | '´' | '`' => out.push('\''),
                    '“' | '”' | '«' | '»' => out.push('"'),
                    '–' | '—' | '−' => out.push('-'),
                    '…' => out.push_str("..."),
                    '\u{00A0}' => out.push(' '),
                    c if c.is_ascii_graphic() || c == ' ' => out.push(c),
                    // Conserve les lettres latines accentuées (Latin-1 Supplement & Extended-A)
                    c if ('\u{00C0}'..='\u{017F}').contains(&c) && c.is_alphabetic() => out.push(c),
                    _ => {}
                }
            }
            collapse_whitespace(&out)
        }
        SanitizeLevel::StrictAscii => {
            let mut out = String::with_capacity(input.len());
            for ch in input.chars() {
                if ch.is_control() || is_emoji_or_symbol_unsafe_for_retro(ch) {
                    continue;
                }
                if let Some(folded) = fold_to_ascii_str(ch) {
                    out.push_str(folded);
                } else if ch.is_ascii_graphic() || ch == ' ' {
                    out.push(ch);
                }
            }
            collapse_whitespace(&out)
        }
    }
}

/// Nettoie un segment de chemin (dossier/fichier) pour FAT32 ou Memory Stick Pro Duo (PSP).
pub fn sanitize_path_segment_for_level(segment: &str, level: SanitizeLevel) -> String {
    let base = sanitize_text(segment, level);
    if level == SanitizeLevel::Standard {
        return base;
    }

    let cleaned: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | ';' | '=' | '+' | '[' | ']' => {
                '_'
            }
            _ => c,
        })
        .collect();

    let trimmed = collapse_whitespace(&cleaned)
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string();

    if trimmed.is_empty() {
        "Inconnu".to_string()
    } else {
        trimmed
    }
}

/// Applique les règles de sanitization d'un profil matériel à une structure `TrackMetadata`.
pub fn sanitize_metadata_for_profile(
    metadata: &TrackMetadata,
    profile: &HardwareProfile,
) -> TrackMetadata {
    let level = SanitizeLevel::for_profile(profile);
    if level == SanitizeLevel::Standard {
        return metadata.clone();
    }

    let sanitize_non_empty = |val: &str, fallback: &str| {
        let s = sanitize_text(val, level);
        if s.is_empty() {
            fallback.to_string()
        } else {
            s
        }
    };

    let mut clean = metadata.clone();
    clean.title = sanitize_non_empty(&metadata.title, "Untitled");
    clean.artist = sanitize_non_empty(&metadata.artist, "Unknown Artist");
    clean.album_artist = sanitize_non_empty(&metadata.album_artist, &clean.artist);
    clean.album = sanitize_non_empty(&metadata.album, &clean.title);
    clean.genre = metadata
        .genre
        .as_deref()
        .map(|g| sanitize_text(g, level))
        .filter(|g| !g.is_empty());

    if *profile == HardwareProfile::Psp {
        // PSP XMB : ID3v2.3 propre sans paroles synchronisées ni .lrc
        clean.lyrics = None;
        clean.synced_lyrics = None;
    }

    clean
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MetadataSource;

    #[test]
    fn sanitizes_emojis_and_special_chars_for_ipod_fat32_while_keeping_accents() {
        let raw = "Beyoncé 🔥 — Étoile d’or ★ (Remix) ✨";
        let sanitized = sanitize_text(raw, SanitizeLevel::Fat32);
        assert_eq!(sanitized, "Beyoncé - Étoile d'or (Remix)");
        assert!(!sanitized.contains('🔥'));
        assert!(!sanitized.contains('★'));
    }

    #[test]
    fn sanitizes_strictly_to_pure_ascii_for_psp_3000() {
        let raw = "Céline Dion 🔥 — Cœur d’été (Édition Spéciale) 日本語";
        let sanitized = sanitize_text(raw, SanitizeLevel::StrictAscii);
        assert_eq!(sanitized, "Celine Dion - Coeur d'ete (Edition Speciale)");
        assert!(sanitized.is_ascii());
    }

    #[test]
    fn sanitizes_fat32_and_psp_path_segments_safely() {
        assert_eq!(
            sanitize_path_segment_for_level("A$AP Rocky: Live? [2024] 🔥", SanitizeLevel::StrictAscii),
            "A$AP Rocky_ Live_ _2024_"
        );
        assert_eq!(
            sanitize_path_segment_for_level("🔥🔥🔥", SanitizeLevel::StrictAscii),
            "Inconnu"
        );
    }

    #[test]
    fn strips_lyrics_and_applies_ascii_rules_on_psp_metadata() {
        let meta = TrackMetadata {
            title: "Été 90 🔥".to_string(),
            artist: "Thérapie TAXI".to_string(),
            album_artist: "Thérapie TAXI".to_string(),
            album: "Cadavre Exquis…".to_string(),
            track_number: Some(1),
            track_total: Some(10),
            disc_number: Some(1),
            disc_total: Some(1),
            year: Some(2019),
            genre: Some("Variété".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: Some(MetadataSource::AppleMusic),
            lyrics: Some("Paroles test".to_string()),
            synced_lyrics: Some("[00:01.00] Paroles test".to_string()),
            bpm: Some(122),
        };

        let psp = sanitize_metadata_for_profile(&meta, &HardwareProfile::Psp);
        assert_eq!(psp.title, "Ete 90");
        assert_eq!(psp.artist, "Therapie TAXI");
        assert_eq!(psp.album, "Cadavre Exquis...");
        assert_eq!(psp.genre.as_deref(), Some("Variete"));
        assert!(psp.lyrics.is_none());
        assert!(psp.synced_lyrics.is_none());
    }
}

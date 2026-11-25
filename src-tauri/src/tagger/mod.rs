//! Moteur de lecture, nettoyage intégral ("Table Rase") et écriture des tags audio
//! via `lofty` (SPECIFICATIONS.md §4.1 & §4.3).
//!
//! Règle de sécurité (§5.1) : `write_clean_tags` ne doit être appelé que sur les copies
//! exportées dans `Documents/Tagmaster_DJ_Export`, jamais sur les fichiers originaux.

pub mod rules;
pub mod sanitizer;

use std::borrow::Cow;
use std::path::Path;

use lofty::TextEncoding;
use lofty::config::WriteOptions;
use lofty::id3::v2::{
    BinaryFrame, Frame, FrameId, Id3v2Tag, SyncTextContentType, SynchronizedTextFrame,
    TimestampFormat,
};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{Tag, TagType};

use crate::api::apple_music::DownloadedArtwork;
use crate::api::genius::{parse_lrc_lines, strip_lrc_timestamps};
use crate::models::{ExistingTrackTags, HardwareProfile, TrackMetadata};
use crate::tagger::rules::looks_like_dirty_tag;

fn non_blank(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn extract_embedded_artwork_to_temp(source_path: &Path, picture: &Picture) -> Option<String> {
    let data = picture.data();
    if data.is_empty() {
        return None;
    }

    let size_bytes = std::fs::metadata(source_path)
        .ok()
        .map(|m| m.len())
        .unwrap_or(0);
    let mut key_buf = source_path.to_string_lossy().as_bytes().to_vec();
    key_buf.extend_from_slice(&size_bytes.to_le_bytes());
    let hash = fnv1a_64(&key_buf);

    let ext = match picture.mime_type() {
        Some(MimeType::Png) => "png",
        Some(MimeType::Unknown(m)) if m.to_ascii_lowercase().contains("webp") => "webp",
        _ => "jpg",
    };

    let cache_dir = std::env::temp_dir().join("tagmaster_dj_embedded_covers");
    std::fs::create_dir_all(&cache_dir).ok()?;
    let dest_path = cache_dir.join(format!("cover_{hash:016x}.{ext}"));

    if !dest_path.is_file() {
        std::fs::write(&dest_path, data).ok()?;
    }

    Some(dest_path.to_string_lossy().into_owned())
}

/// Lecture seule des tags existants sur un fichier source (ne modifie jamais le fichier).
pub fn read_existing_tags(path: &Path) -> Result<ExistingTrackTags, String> {
    let tagged_file = Probe::open(path)
        .map_err(|err| format!("Impossible d'ouvrir {} : {err}", path.display()))?
        .guess_file_type()
        .map_err(|err| format!("Format audio non reconnu pour {} : {err}", path.display()))?
        .read()
        .map_err(|err| format!("Impossible de lire les tags de {} : {err}", path.display()))?;

    let all_tags = tagged_file.tags();
    let tag_count = all_tags.len();

    let mut has_dirty_tags = false;
    for tag in all_tags {
        for item in tag.items() {
            if let Some(text) = item.value().text()
                && looks_like_dirty_tag(text)
            {
                has_dirty_tags = true;
                break;
            }
        }
        if has_dirty_tags {
            break;
        }
    }

    let primary = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
    let mut title = None;
    let mut artist = None;
    let mut album_artist = None;
    let mut album = None;
    let mut embedded_picture: Option<&Picture> = None;

    for tag in primary.into_iter().chain(all_tags.iter()) {
        if title.is_none() {
            title = non_blank(tag.title().map(|s| s.into_owned()));
        }
        if artist.is_none() {
            artist = non_blank(tag.artist().map(|s| s.into_owned()));
        }
        if album_artist.is_none() {
            album_artist = non_blank(tag.get_string(ItemKey::AlbumArtist).map(str::to_string));
        }
        if album.is_none() {
            album = non_blank(tag.album().map(|s| s.into_owned()));
        }
        if embedded_picture.is_none() {
            embedded_picture = tag
                .pictures()
                .iter()
                .find(|p| p.pic_type() == PictureType::CoverFront && !p.data().is_empty())
                .or_else(|| tag.pictures().iter().find(|p| !p.data().is_empty()));
        }
        if title.is_some()
            && artist.is_some()
            && album_artist.is_some()
            && album.is_some()
            && embedded_picture.is_some()
        {
            break;
        }
    }

    let embedded_artwork_path =
        embedded_picture.and_then(|pic| extract_embedded_artwork_to_temp(path, pic));

    Ok(ExistingTrackTags {
        title,
        artist,
        album_artist,
        album,
        tag_count,
        has_dirty_tags,
        embedded_artwork_path,
    })
}

/// Phase 1 (« Table Rase ») : supprime intégralement tous les conteneurs de tags présents
/// (ID3v1, ID3v2, APEv2, MP4, Vorbis, RIFF…) afin d'éradiquer commentaires publicitaires
/// et encodeurs parasites (`www.mediahuman.com`, `free-mp3`, etc.).
pub fn wipe_all_tags(target_path: &Path) -> Result<TagType, String> {
    let tagged_file = Probe::open(target_path)
        .map_err(|err| format!("Impossible d'ouvrir {} : {err}", target_path.display()))?
        .guess_file_type()
        .map_err(|err| format!("Type de fichier inconnu pour {} : {err}", target_path.display()))?
        .read()
        .map_err(|err| format!("Lecture impossible pour {} : {err}", target_path.display()))?;

    let primary_type = tagged_file.primary_tag_type();
    let existing_types: Vec<TagType> = tagged_file.tags().iter().map(|t| t.tag_type()).collect();
    drop(tagged_file);

    let write_opts = WriteOptions::default();
    for tag_type in existing_types {
        tag_type
            .remove_from_path(target_path, write_opts)
            .map_err(|err| {
                format!(
                    "Impossible de supprimer l'ancien tag {:?} de {} : {err}",
                    tag_type,
                    target_path.display()
                )
            })?;
    }

    Ok(primary_type)
}

/// Phase 2 : exécute la Table Rase puis écrit un conteneur de tags neuf et normalisé
/// conforme à `SPECIFICATIONS.md` §4.3 (+ pochette HD binaire si disponible).
pub fn write_clean_tags(
    target_path: &Path,
    metadata: &TrackMetadata,
    artwork: Option<&DownloadedArtwork>,
) -> Result<(), String> {
    write_clean_tags_for_profile(target_path, metadata, artwork, &HardwareProfile::Standard)
}

/// Phase 2 avec prise en charge du profil matériel (`Standard` = ID3v2.4 + SYLT/USLT,
/// `iPod` = ID3v2.3 + USLT, `PSP 3000` = ID3v2.3 strict sans SYLT/USLT).
pub fn write_clean_tags_for_profile(
    target_path: &Path,
    metadata: &TrackMetadata,
    artwork: Option<&DownloadedArtwork>,
    profile: &HardwareProfile,
) -> Result<(), String> {
    let primary_type = wipe_all_tags(target_path)?;
    let mut tag = Tag::new(primary_type);
    let write_opts = match profile {
        HardwareProfile::Standard => WriteOptions::default(),
        HardwareProfile::Ipod | HardwareProfile::Psp => WriteOptions::new().use_id3v23(true),
    };

    tag.set_title(metadata.title.clone());
    tag.set_artist(metadata.artist.clone());
    tag.set_album(metadata.album.clone());
    tag.insert_text(ItemKey::AlbumArtist, metadata.album_artist.clone());

    if let Some(genre) = &metadata.genre
        && !genre.is_empty()
    {
        tag.set_genre(genre.clone());
    }

    if let Some(track) = metadata.track_number {
        tag.set_track(track);
    }
    if let Some(track_total) = metadata.track_total {
        tag.set_track_total(track_total);
    }

    if let Some(disc) = metadata.disc_number {
        tag.set_disk(disc);
    }
    if let Some(disc_total) = metadata.disc_total {
        tag.set_disk_total(disc_total);
    }

    if let Some(year) = metadata.year {
        tag.insert_text(ItemKey::Year, year.to_string());
    }

    if metadata.is_compilation {
        tag.insert_text(ItemKey::FlagCompilation, "1".to_string());
    }

    if let Some(art) = artwork
        && !art.bytes.is_empty()
    {
        let mime = if art.mime_type.contains("png") {
            MimeType::Png
        } else if art.mime_type.contains("webp") {
            MimeType::Unknown("image/webp".to_string())
        } else {
            MimeType::Jpeg
        };

        let picture = Picture::unchecked(art.bytes.clone())
            .pic_type(PictureType::CoverFront)
            .mime_type(mime)
            .build();
        tag.push_picture(picture);
    }

    let allow_lyrics = *profile != HardwareProfile::Psp;
    let allow_sylt = *profile == HardwareProfile::Standard;

    if allow_lyrics {
        let plain_lyrics = metadata
            .lyrics
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                metadata
                    .synced_lyrics
                    .as_deref()
                    .map(strip_lrc_timestamps)
                    .filter(|s| !s.is_empty())
            });

        let synced_raw = metadata
            .synced_lyrics
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        if primary_type == TagType::Id3v2 {
            if let Some(plain) = plain_lyrics {
                tag.insert_text(ItemKey::UnsyncLyrics, plain);
            }

            if allow_sylt
                && let Some(lrc) = synced_raw
            {
                let sylt_entries = parse_lrc_lines(lrc);
                if !sylt_entries.is_empty() {
                    let mut id3v2: Id3v2Tag = tag.into();
                    let sylt = SynchronizedTextFrame::new(
                        TextEncoding::UTF8,
                        *b"XXX",
                        TimestampFormat::MS,
                        SyncTextContentType::Lyrics,
                        None,
                        sylt_entries,
                    );
                    let bytes = sylt.as_bytes(write_opts).map_err(|err| {
                        format!(
                            "Impossible d'encoder les paroles synchronisées SYLT pour {} : {err}",
                            target_path.display()
                        )
                    })?;
                    id3v2.insert(Frame::Binary(BinaryFrame::new(
                        FrameId::Valid(Cow::Borrowed("SYLT")),
                        bytes,
                    )));
                    return id3v2.save_to_path(target_path, write_opts).map_err(|err| {
                        format!(
                            "Impossible d'écrire les nouveaux tags ID3v2 (+SYLT) sur {} : {err}",
                            target_path.display()
                        )
                    });
                }
            }
        } else if let Some(best_lyrics) = synced_raw.map(str::to_string).or(plain_lyrics) {
            tag.insert_text(ItemKey::Lyrics, best_lyrics);
        }
    }

    tag.save_to_path(target_path, write_opts).map_err(|err| {
        format!(
            "Impossible d'écrire les nouveaux tags sur {} : {err}",
            target_path.display()
        )
    })
}

/// Charge une pochette locale (`.jpg`, `.jpeg`, `.png`, `.webp`) sélectionnée manuellement.
pub fn load_local_artwork(path: &Path) -> Result<DownloadedArtwork, String> {
    if !path.is_file() {
        return Err(format!(
            "Fichier pochette introuvable : {}",
            path.display()
        ));
    }

    let bytes = std::fs::read(path).map_err(|err| {
        format!(
            "Impossible de lire la pochette locale {} : {err}",
            path.display()
        )
    })?;

    if bytes.is_empty() {
        return Err(format!("La pochette locale {} est vide.", path.display()));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let mime_type = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) || ext == "png" {
        "image/png".to_string()
    } else if (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP")
        || ext == "webp"
    {
        "image/webp".to_string()
    } else {
        "image/jpeg".to_string()
    };

    Ok(DownloadedArtwork { bytes, mime_type })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MetadataSource;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Construit un fichier MP3 minimal valide (frames MPEG-1 Layer III 128kbps 44.1kHz)
    /// pour tester la lecture, la Table Rase et la réécriture complète via `lofty`.
    fn create_minimal_valid_mp3() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock error")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tagmaster_dj_tag_test_{nanos}.mp3"));

        // Header MPEG1 Layer III, no CRC, 128 kbps (0x90), 44100 Hz (0x00), Joint Stereo (0x64)
        // Taille d'une frame : floor(144 * 128000 / 44100) = 417 bytes
        let mut frame = vec![0u8; 417];
        frame[0] = 0xFF;
        frame[1] = 0xFB;
        frame[2] = 0x90;
        frame[3] = 0x64;

        let mut data = Vec::with_capacity(417 * 4);
        for _ in 0..4 {
            data.extend_from_slice(&frame);
        }

        fs::write(&path, data).expect("write synthetic mp3");
        path
    }

    #[test]
    fn table_rase_wipes_dirty_id3v1_and_id3v2_and_writes_clean_tags_with_artwork() {
        let mp3_path = create_minimal_valid_mp3();

        // 1. Injecter des tags sales ID3v2 + ID3v1 (commentaire mediahuman + encodeur parasite)
        let mut dirty_v2 = Tag::new(TagType::Id3v2);
        dirty_v2.set_title("01_bad_gyal_chulo_www.mediahuman.com".into());
        dirty_v2.set_comment("Downloaded via www.mediahuman.com free-mp3".into());
        dirty_v2.insert_text(ItemKey::EncoderSoftware, "free-mp3-ripper-v1".into());
        dirty_v2
            .save_to_path(&mp3_path, WriteOptions::default())
            .expect("save dirty v2");

        let mut dirty_v1 = Tag::new(TagType::Id3v1);
        dirty_v1.set_comment("www.mediahuman.com".into());
        dirty_v1
            .save_to_path(&mp3_path, WriteOptions::default())
            .expect("save dirty v1");

        // Vérifier que la détection en lecture seule repère bien les tags sales
        let before = read_existing_tags(&mp3_path).expect("read dirty tags");
        assert!(before.has_dirty_tags);
        assert_eq!(before.tag_count, 2);

        // 2. Exécuter Table Rase + écriture propre avec pochette binaire
        let clean_meta = TrackMetadata {
            title: "Chulo pt.2 (feat. Tokischa & Young Miko)".to_string(),
            artist: "Bad Gyal".to_string(),
            album_artist: "Bad Gyal".to_string(),
            album: "La Joia".to_string(),
            track_number: Some(4),
            track_total: Some(15),
            disc_number: Some(1),
            disc_total: Some(2),
            year: Some(2024),
            genre: Some("Urbano latino".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: Some(MetadataSource::AppleMusic),
            lyrics: None,
            synced_lyrics: None,
        };

        let fake_jpeg = DownloadedArtwork {
            bytes: vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46],
            mime_type: "image/jpeg".to_string(),
        };

        write_clean_tags(&mp3_path, &clean_meta, Some(&fake_jpeg))
            .expect("write_clean_tags should succeed");

        // 3. Relire le fichier depuis zéro et vérifier la propreté intégrale
        let after_summary = read_existing_tags(&mp3_path).expect("read clean tags");
        assert!(!after_summary.has_dirty_tags);
        assert_eq!(after_summary.tag_count, 1, "ID3v1 sale doit être supprimé");
        if let Some(temp_art) = after_summary.embedded_artwork_path.as_deref() {
            assert!(Path::new(temp_art).is_file());
            let _ = fs::remove_file(temp_art);
        } else {
            panic!("embedded_artwork_path doit être extrait en lecture seule");
        }

        let reread = Probe::open(&mp3_path)
            .unwrap()
            .guess_file_type()
            .unwrap()
            .read()
            .unwrap();
        let tag = reread.primary_tag().expect("primary ID3v2 tag expected");

        assert_eq!(
            tag.title().as_deref(),
            Some("Chulo pt.2 (feat. Tokischa & Young Miko)")
        );
        assert_eq!(tag.artist().as_deref(), Some("Bad Gyal"));
        assert_eq!(tag.get_string(ItemKey::AlbumArtist), Some("Bad Gyal"));
        assert_eq!(tag.album().as_deref(), Some("La Joia"));
        assert_eq!(tag.track(), Some(4));
        assert_eq!(tag.track_total(), Some(15));
        assert_eq!(tag.disk(), Some(1));
        assert_eq!(tag.disk_total(), Some(2));
        assert_eq!(tag.genre().as_deref(), Some("Urbano latino"));
        assert_eq!(tag.comment(), None, "Commentaire publicitaire doit être purgé");
        assert_eq!(
            tag.get_string(ItemKey::EncoderSoftware),
            None,
            "Encodeur parasite doit être purgé"
        );
        assert_eq!(tag.pictures().len(), 1);
        assert_eq!(tag.pictures()[0].pic_type(), PictureType::CoverFront);
        assert_eq!(tag.pictures()[0].data(), fake_jpeg.bytes.as_slice());

        let _ = fs::remove_file(&mp3_path);
    }

    #[test]
    fn reads_existing_tags_including_album_artist_without_modifying_source() {
        let mp3_path = create_minimal_valid_mp3();

        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("05 T mimi".into());
        tag.set_artist("Shay".into());
        tag.insert_text(ItemKey::AlbumArtist, "Shay".into());
        tag.set_album("Pourvu qu'il pleuve".into());
        tag.save_to_path(&mp3_path, WriteOptions::default())
            .expect("save id3v2 tags");

        let before_bytes = fs::read(&mp3_path).expect("read original bytes");
        let existing = read_existing_tags(&mp3_path).expect("read_existing_tags should succeed");
        let after_bytes = fs::read(&mp3_path).expect("re-read bytes");

        assert_eq!(before_bytes, after_bytes, "source file must remain untouched");
        assert_eq!(existing.title.as_deref(), Some("05 T mimi"));
        assert_eq!(existing.artist.as_deref(), Some("Shay"));
        assert_eq!(existing.album_artist.as_deref(), Some("Shay"));
        assert_eq!(existing.album.as_deref(), Some("Pourvu qu'il pleuve"));
        assert!(!existing.has_dirty_tags);

        let _ = fs::remove_file(&mp3_path);
    }

    #[test]
    fn writes_genius_lyrics_into_id3v2_tag() {
        let mp3_path = create_minimal_valid_mp3();
        let meta = TrackMetadata {
            title: "T mimi".to_string(),
            artist: "Shay".to_string(),
            album_artist: "Shay".to_string(),
            album: "Pourvu qu'il pleuve".to_string(),
            track_number: Some(5),
            track_total: Some(15),
            disc_number: Some(1),
            disc_total: Some(1),
            year: Some(2024),
            genre: Some("Hip-Hop/Rap".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: Some(MetadataSource::AppleMusic),
            lyrics: Some("[Couplet 1]\nT mimi comme tout".to_string()),
            synced_lyrics: None,
        };

        write_clean_tags(&mp3_path, &meta, None).expect("write_clean_tags with lyrics should pass");

        let reread = Probe::open(&mp3_path)
            .unwrap()
            .guess_file_type()
            .unwrap()
            .read()
            .unwrap();
        let tag = reread.primary_tag().expect("primary ID3v2 tag expected");
        assert_eq!(
            tag.get_string(ItemKey::UnsyncLyrics),
            Some("[Couplet 1]\nT mimi comme tout")
        );

        let _ = fs::remove_file(&mp3_path);
    }

    #[test]
    fn writes_synchronized_sylt_and_uslt_lyrics_into_mp3() {
        use lofty::config::ParseOptions;
        use lofty::mpeg::MpegFile;

        let mp3_path = create_minimal_valid_mp3();
        let lrc = "[00:01.25] Première ligne\n[00:05.50] Deuxième ligne";
        let meta = TrackMetadata {
            title: "Gatitas".to_string(),
            artist: "Bad Gyal".to_string(),
            album_artist: "Bad Gyal".to_string(),
            album: "Más Cara".to_string(),
            track_number: Some(1),
            track_total: Some(12),
            disc_number: Some(1),
            disc_total: Some(1),
            year: Some(2026),
            genre: Some("Latin".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: Some(MetadataSource::AppleMusic),
            lyrics: Some("Première ligne\nDeuxième ligne".to_string()),
            synced_lyrics: Some(lrc.to_string()),
        };

        let fake_jpeg = DownloadedArtwork {
            bytes: vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46],
            mime_type: "image/jpeg".to_string(),
        };

        write_clean_tags(&mp3_path, &meta, Some(&fake_jpeg))
            .expect("write_clean_tags with SYLT and artwork should pass");

        let reread = Probe::open(&mp3_path)
            .unwrap()
            .guess_file_type()
            .unwrap()
            .read()
            .unwrap();
        let primary = reread.primary_tag().expect("primary ID3v2 tag expected");
        assert_eq!(primary.pictures().len(), 1, "APIC artwork must be preserved alongside SYLT");
        assert_eq!(primary.pictures()[0].data(), fake_jpeg.bytes.as_slice());
        assert_eq!(
            primary.get_string(ItemKey::UnsyncLyrics),
            Some("Première ligne\nDeuxième ligne")
        );

        let mut file = fs::File::open(&mp3_path).expect("open tagged mp3");
        let mpeg = MpegFile::read_from(&mut file, ParseOptions::new()).expect("parse mp3");
        let id3v2 = mpeg.id3v2().expect("id3v2 tag expected");
        let sylt_frame = id3v2
            .get(&FrameId::Valid(Cow::Borrowed("SYLT")))
            .expect("SYLT frame should exist");

        let Frame::Binary(bin) = sylt_frame else {
            panic!("SYLT frame should be parsed as Frame::Binary");
        };
        let parsed_sylt = SynchronizedTextFrame::parse(&bin.data, bin.flags())
            .expect("SYLT binary frame should decode cleanly");
        assert_eq!(
            parsed_sylt.content,
            vec![
                (1250, "Première ligne".to_string()),
                (5500, "Deuxième ligne".to_string()),
            ]
        );

        let _ = fs::remove_file(&mp3_path);
    }
}

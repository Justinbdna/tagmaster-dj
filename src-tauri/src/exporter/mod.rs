//! Module d'export sécurisé non destructif (SPECIFICATIONS.md §5).
//!
//! Ne jamais écraser les fichiers originaux : tous les exports sont écrits dans
//! `Documents/Tagmaster_DJ_Export` en mode Plat ou Dynamique après copie préalable.

pub mod path_guard;

use std::fs;
use std::path::{Path, PathBuf};

use crate::api::apple_music::DownloadedArtwork;
use crate::ffmpeg;
use crate::models::{ExportFormatMode, ExportMode, HardwareProfile, TrackMetadata};
use crate::tagger;
use crate::tagger::sanitizer::sanitize_metadata_for_profile;

pub const DEFAULT_EXPORT_DIR_NAME: &str = "Tagmaster_DJ_Export";

/// Exporte une piste de manière strictement non destructive (profil `Standard`, mode `Passthrough`).
pub fn export_single_track(
    source_path: &Path,
    export_root: &Path,
    mode: ExportMode,
    metadata: &TrackMetadata,
    artwork: Option<&DownloadedArtwork>,
) -> Result<PathBuf, String> {
    export_single_track_with_profile(
        source_path,
        export_root,
        mode,
        ExportFormatMode::Passthrough,
        HardwareProfile::Standard,
        metadata,
        artwork,
    )
}

/// Exporte une piste selon le mode d'arborescence, le format audio (Passthrough / Force MP3 320k)
/// et le profil matériel (`Standard`, `iPod`, `PSP`).
pub fn export_single_track_with_profile(
    source_path: &Path,
    export_root: &Path,
    mode: ExportMode,
    format_mode: ExportFormatMode,
    hardware_profile: HardwareProfile,
    metadata: &TrackMetadata,
    artwork: Option<&DownloadedArtwork>,
) -> Result<PathBuf, String> {
    if !source_path.is_file() {
        return Err(format!(
            "Fichier source introuvable : {}",
            source_path.display()
        ));
    }

    let source_ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3");

    let needs_mp3_transcode =
        ffmpeg::should_transcode_to_mp3(&hardware_profile, &format_mode, source_ext);
    let target_ext = if needs_mp3_transcode {
        "mp3"
    } else {
        source_ext
    };

    let clean_metadata = sanitize_metadata_for_profile(metadata, &hardware_profile);

    let desired_path = path_guard::build_export_path_for_profile(
        export_root,
        mode,
        &clean_metadata,
        target_ext,
        &hardware_profile,
    );
    let dest_path = path_guard::resolve_non_colliding_path(&desired_path, source_path);

    if let (Ok(canon_src), Ok(canon_dst)) =
        (fs::canonicalize(source_path), fs::canonicalize(&dest_path))
        && canon_src == canon_dst
    {
        return Err("Sécurité : le chemin d'export cible correspond au fichier source.".to_string());
    }

    let parent = dest_path.parent().ok_or_else(|| {
        format!(
            "Impossible de déterminer le dossier parent pour {}",
            dest_path.display()
        )
    })?;

    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "Impossible de créer le dossier d'export {} : {err}",
            parent.display()
        )
    })?;

    if needs_mp3_transcode {
        ffmpeg::transcode_to_mp3_320(source_path, &dest_path).map_err(|err| {
            let _ = fs::remove_file(&dest_path);
            err.to_string()
        })?;
    } else {
        fs::copy(source_path, &dest_path).map_err(|err| {
            format!(
                "Impossible de copier {} vers {} : {err}",
                source_path.display(),
                dest_path.display()
            )
        })?;
    }

    let prepared_artwork = ffmpeg::prepare_artwork_for_profile(artwork, &hardware_profile);

    if let Err(err) = tagger::write_clean_tags_for_profile(
        &dest_path,
        &clean_metadata,
        prepared_artwork.as_ref(),
        &hardware_profile,
    ) {
        let _ = fs::remove_file(&dest_path);
        return Err(err);
    }

    if hardware_profile == HardwareProfile::Standard
        && let Some(synced) = clean_metadata
            .synced_lyrics
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    {
        let lrc_path = dest_path.with_extension("lrc");
        let _ = fs::write(lrc_path, format!("{synced}\n"));
    }

    Ok(dest_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MetadataSource;
    use lofty::config::WriteOptions;
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use lofty::tag::{Tag, TagType};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_dirty_mp3_fixture(dir: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(dir).expect("create source dir");
        let path = dir.join(name);

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

        let mut dirty_tag = Tag::new(TagType::Id3v2);
        dirty_tag.set_comment("Downloaded from www.mediahuman.com".to_string());
        dirty_tag.set_title("dirty_raw_title".to_string());
        dirty_tag
            .save_to_path(&path, WriteOptions::default())
            .expect("inject dirty tag");

        path
    }

    #[test]
    fn exports_non_destructively_preserving_source_and_writing_full_internal_tags() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock error")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("tagmaster_dj_export_test_{nanos}"));
        let source_dir = base_dir.join("source");
        let export_dir = base_dir.join("Tagmaster_DJ_Export");

        let source_file = create_dirty_mp3_fixture(&source_dir, "01_dirty_rip.mp3");
        let original_bytes = fs::read(&source_file).expect("read original bytes");

        let long_full_title = format!(
            "Epic Track Version {}",
            "Extraordinary Symphony Part ".repeat(10)
        );
        let meta = TrackMetadata {
            title: long_full_title.clone(),
            artist: "Bad Gyal".to_string(),
            album_artist: "Bad Gyal".to_string(),
            album: "La Joia Deluxe Edition".to_string(),
            track_number: Some(3),
            track_total: Some(12),
            disc_number: Some(1),
            disc_total: Some(2),
            year: Some(2024),
            genre: Some("Urbano latino".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: Some(MetadataSource::AppleMusic),
            lyrics: None,
            synced_lyrics: Some("[00:05.00] Linea sincronizada".to_string()),
        };

        let exported_path = export_single_track(
            &source_file,
            &export_dir,
            ExportMode::Dynamic,
            &meta,
            None,
        )
        .expect("export should succeed");

        // 1. Le fichier original doit être strictement intact octet par octet
        let source_bytes_after = fs::read(&source_file).expect("re-read original bytes");
        assert_eq!(
            original_bytes, source_bytes_after,
            "original file must never be altered"
        );

        // 2. Le chemin physique respecte la limite Windows et se termine par ....mp3
        let exported_len = exported_path.to_string_lossy().chars().count();
        assert!(exported_len <= path_guard::WINDOWS_MAX_PATH_SAFE);
        assert!(exported_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("....mp3"));

        // 3. Les tags internes conservent le titre complet et sont purgés de mediahuman
        let reread = Probe::open(&exported_path)
            .unwrap()
            .guess_file_type()
            .unwrap()
            .read()
            .unwrap();
        let clean_tag = reread.primary_tag().expect("clean tag expected");
        assert_eq!(clean_tag.title().as_deref(), Some(long_full_title.as_str()));
        assert_eq!(clean_tag.get_string(ItemKey::AlbumArtist), Some("Bad Gyal"));
        assert_eq!(clean_tag.comment(), None);

        let lrc_path = exported_path.with_extension("lrc");
        assert!(lrc_path.is_file(), "sidecar .lrc file should be exported alongside track");
        assert_eq!(
            fs::read_to_string(&lrc_path).unwrap(),
            "[00:05.00] Linea sincronizada\n"
        );

        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn exports_ipod_profile_with_fat32_sanitization_and_no_lrc_sidecar() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock error")
            .as_nanos();
        let base_dir = std::env::temp_dir().join(format!("tagmaster_dj_ipod_test_{nanos}"));
        let source_dir = base_dir.join("source");
        let export_dir = base_dir.join("Tagmaster_DJ_Export");

        let source_file = create_dirty_mp3_fixture(&source_dir, "ipod_source.mp3");
        let meta = TrackMetadata {
            title: "Étoile 🔥".to_string(),
            artist: "Beyoncé ★".to_string(),
            album_artist: "Beyoncé ★".to_string(),
            album: "Renaissance ✨".to_string(),
            track_number: Some(1),
            track_total: Some(16),
            disc_number: Some(1),
            disc_total: Some(1),
            year: Some(2022),
            genre: Some("Pop".to_string()),
            artwork_url: None,
            is_compilation: false,
            source: Some(MetadataSource::AppleMusic),
            lyrics: Some("Hello iPod".to_string()),
            synced_lyrics: Some("[00:01.00] Hello iPod".to_string()),
        };

        let exported_path = export_single_track_with_profile(
            &source_file,
            &export_dir,
            ExportMode::Flat,
            ExportFormatMode::Passthrough,
            HardwareProfile::Ipod,
            &meta,
            None,
        )
        .expect("iPod export should succeed");

        let file_name = exported_path.file_name().unwrap().to_string_lossy();
        assert_eq!(file_name, "Beyoncé - Étoile.mp3");
        assert!(
            !exported_path.with_extension("lrc").exists(),
            "iPod profile must not write .lrc sidecar"
        );

        let reread = Probe::open(&exported_path)
            .unwrap()
            .guess_file_type()
            .unwrap()
            .read()
            .unwrap();
        let tag = reread.primary_tag().expect("ID3v2.3 tag expected");
        assert_eq!(tag.title().as_deref(), Some("Étoile"));
        assert_eq!(tag.artist().as_deref(), Some("Beyoncé"));
        assert_eq!(tag.get_string(ItemKey::UnsyncLyrics), Some("Hello iPod"));

        let _ = fs::remove_dir_all(&base_dir);
    }
}

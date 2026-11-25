//! Module d'export sécurisé non destructif (SPECIFICATIONS.md §5).
//!
//! Ne jamais écraser les fichiers originaux : tous les exports sont écrits dans
//! `Documents/Tagmaster_DJ_Export` en mode Plat ou Dynamique après copie préalable.

pub mod path_guard;

use std::fs;
use std::path::{Path, PathBuf};

use crate::api::apple_music::DownloadedArtwork;
use crate::models::{ExportMode, TrackMetadata};
use crate::tagger;

pub const DEFAULT_EXPORT_DIR_NAME: &str = "Tagmaster_DJ_Export";

/// Exporte une piste de manière strictement non destructive :
/// 1. Calcule le chemin cible Plat ou Dynamique sous la limite Windows (256 car.).
/// 2. Copie le fichier source vers `export_root` (l'original n'est jamais modifié).
/// 3. Applique le nettoyage « Table Rase » et écrit les nouveaux tags + pochette HD sur la copie.
pub fn export_single_track(
    source_path: &Path,
    export_root: &Path,
    mode: ExportMode,
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

    let desired_path = path_guard::build_export_path(export_root, mode, metadata, source_ext);
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

    fs::copy(source_path, &dest_path).map_err(|err| {
        format!(
            "Impossible de copier {} vers {} : {err}",
            source_path.display(),
            dest_path.display()
        )
    })?;

    if let Err(err) = tagger::write_clean_tags(&dest_path, metadata, artwork) {
        let _ = fs::remove_file(&dest_path);
        return Err(err);
    }

    if let Some(synced) = metadata
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
}

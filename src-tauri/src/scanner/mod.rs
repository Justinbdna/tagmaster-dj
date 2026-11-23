//! Scanner récursif de fichiers audio (SPECIFICATIONS.md §3 & §7).
//!
//! Prend en charge un mélange de dossiers et de fichiers individuels provenant
//! du sélecteur natif (`tauri-plugin-dialog`) ou du Drag & Drop Tauri.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::models::{AudioTrackItem, ScanResult, TrackStatus};
use crate::tagger::rules::contains_protected_keyword;

pub const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "aac", "flac", "wav"];

fn is_hidden_or_resource_fork(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.starts_with('.') || name.starts_with("._"))
        .unwrap_or(false)
}

fn normalized_audio_extension(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if SUPPORTED_AUDIO_EXTENSIONS.contains(&ext.as_str()) {
        Some(ext.to_ascii_uppercase())
    } else {
        None
    }
}

fn build_track_item(path: &Path, extension: String) -> Option<AudioTrackItem> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    let file_name = path.file_name()?.to_string_lossy().into_owned();
    let parent_folder = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Dossier racine".to_string());

    let source_path = path.to_string_lossy().into_owned();
    let has_protected_keyword = contains_protected_keyword(&file_name);

    Some(AudioTrackItem {
        id: source_path.to_lowercase(),
        source_path,
        file_name,
        parent_folder,
        extension,
        size_bytes: metadata.len(),
        has_protected_keyword,
        status: TrackStatus::Pending,
        metadata: None,
        review_reason: None,
    })
}

fn process_file(
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    tracks: &mut Vec<AudioTrackItem>,
    skipped_count: &mut usize,
) {
    if is_hidden_or_resource_fork(path) {
        *skipped_count += 1;
        return;
    }

    let dedup_key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(dedup_key) {
        return;
    }

    match normalized_audio_extension(path) {
        Some(ext) => {
            if let Some(track) = build_track_item(path, ext) {
                tracks.push(track);
            } else {
                *skipped_count += 1;
            }
        }
        None => {
            *skipped_count += 1;
        }
    }
}

pub fn scan_audio_paths(input_paths: &[String]) -> Result<ScanResult, String> {
    let mut tracks = Vec::new();
    let mut skipped_count: usize = 0;
    let mut scanned_folders = Vec::new();
    let mut seen_files: HashSet<PathBuf> = HashSet::new();

    for raw in input_paths {
        let path = PathBuf::from(raw);
        if !path.exists() {
            skipped_count += 1;
            continue;
        }

        if path.is_dir() {
            scanned_folders.push(path.to_string_lossy().into_owned());

            for entry in WalkDir::new(&path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    // Ne jamais filtrer le dossier racine explicitement sélectionné par l'utilisateur
                    e.depth() == 0 || !is_hidden_or_resource_fork(e.path())
                })
            {
                match entry {
                    Ok(dir_entry) if dir_entry.file_type().is_file() => {
                        process_file(
                            dir_entry.path(),
                            &mut seen_files,
                            &mut tracks,
                            &mut skipped_count,
                        );
                    }
                    Ok(_) => {}
                    Err(_) => {
                        skipped_count += 1;
                    }
                }
            }
        } else if path.is_file() {
            process_file(&path, &mut seen_files, &mut tracks, &mut skipped_count);
        }
    }

    tracks.sort_by(|a, b| {
        a.parent_folder
            .to_lowercase()
            .cmp(&b.parent_folder.to_lowercase())
            .then_with(|| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()))
    });

    Ok(ScanResult {
        tracks,
        skipped_count,
        scanned_folders,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_fixture() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock error")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tagmaster_dj_scan_test_{nanos}"));
        let sub = root.join("Album Deluxe");
        fs::create_dir_all(&sub).expect("create temp dirs");

        fs::write(root.join("Bad Gyal - Chulo.mp3"), b"fake-mp3-data").unwrap();
        fs::write(
            sub.join("Daft Punk - Harder Better (Live).FLAC"),
            b"fake-flac-data",
        )
        .unwrap();
        fs::write(sub.join("cover.jpg"), b"image-bytes").unwrap();
        fs::write(sub.join("._AppleDouble.mp3"), b"macos-junk").unwrap();

        root
    }

    #[test]
    fn scans_recursively_filters_non_audio_and_detects_protected_keywords() {
        let root = create_temp_fixture();
        let file_also_passed = root.join("Bad Gyal - Chulo.mp3");

        let result = scan_audio_paths(&[
            root.to_string_lossy().into_owned(),
            file_also_passed.to_string_lossy().into_owned(),
        ])
        .expect("scan should succeed");

        assert_eq!(result.scanned_folders.len(), 1);
        assert_eq!(result.tracks.len(), 2, "duplicate file must be deduplicated");
        assert!(
            result.skipped_count >= 1,
            "non-audio cover.jpg should be counted as skipped"
        );

        let live_track = result
            .tracks
            .iter()
            .find(|t| t.extension == "FLAC")
            .expect("FLAC track expected");
        assert!(live_track.has_protected_keyword);
        assert_eq!(live_track.parent_folder, "Album Deluxe");

        let studio_track = result
            .tracks
            .iter()
            .find(|t| t.extension == "MP3")
            .expect("MP3 track expected");
        assert!(!studio_track.has_protected_keyword);

        let _ = fs::remove_dir_all(&root);
    }
}

//! Persistance d'état locale `cache.json` (SPECIFICATIONS.md §6 · Issue #13).
//!
//! Sauvegarde atomiquement l'état de la file (pistes importées, tags identifiés,
//! statuts, pochettes choisies, mode d'export) afin de permettre une reprise
//! instantanée après coupure réseau ou redémarrage de l'application.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::Manager;

use crate::models::{ExportMode, SessionCache, SessionCacheSummary, TrackStatus};

pub const CACHE_FILENAME: &str = "cache.json";
pub const SESSION_CACHE_VERSION: u32 = 1;

static CACHE_IO_LOCK: Mutex<()> = Mutex::new(());

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Résout le chemin de `cache.json` (racine du dépôt en dev, `app_data_dir` en production).
pub fn resolve_cache_file_path(app: Option<&tauri::AppHandle>) -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let direct = cwd.join(CACHE_FILENAME);
        if direct.is_file() {
            return direct;
        }

        if cwd.file_name().and_then(|n| n.to_str()) == Some("src-tauri")
            && let Some(parent) = cwd.parent()
            && (parent.join("package.json").is_file() || parent.join(".gitignore").is_file())
        {
            return parent.join(CACHE_FILENAME);
        }

        if cwd.join("src-tauri").is_dir() && cwd.join("package.json").is_file() {
            return direct;
        }
    }

    if let Some(handle) = app
        && let Ok(data_dir) = handle.path().app_data_dir()
    {
        return data_dir.join(CACHE_FILENAME);
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(CACHE_FILENAME)
}

/// Nettoie les statuts transitoires (`Processing` -> `Pending`) avant persistance ou reprise.
fn sanitize_session(mut session: SessionCache) -> SessionCache {
    session.version = SESSION_CACHE_VERSION;
    if session.updated_at_ms == 0 {
        session.updated_at_ms = now_epoch_ms();
    }
    for track in &mut session.tracks {
        if track.status == TrackStatus::Processing {
            track.status = TrackStatus::Pending;
        }
    }
    session
}

/// Sauvegarde atomique et sérialisée de la session dans `cache.json` (écriture dans `.tmp` puis renommage).
pub fn save_session_cache_to_path(path: &Path, session: &SessionCache) -> Result<(), String> {
    let _guard = CACHE_IO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if session.tracks.is_empty() {
        return clear_session_cache_at_path_unlocked(path);
    }

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Impossible de créer le dossier de cache {} : {err}",
                parent.display()
            )
        })?;
    }

    let mut clean = sanitize_session(session.clone());
    clean.updated_at_ms = now_epoch_ms();

    let json = serde_json::to_string_pretty(&clean)
        .map_err(|err| format!("Impossible de sérialiser cache.json : {err}"))?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &json).map_err(|err| {
        format!(
            "Impossible d'écrire le fichier temporaire {} : {err}",
            tmp_path.display()
        )
    })?;

    if fs::rename(&tmp_path, path).is_err() {
        fs::write(path, &json).map_err(|err| {
            format!(
                "Impossible d'écrire le cache de session {} : {err}",
                path.display()
            )
        })?;
        let _ = fs::remove_file(&tmp_path);
    }

    Ok(())
}

/// Charge une session précédente depuis `cache.json`. Retourne `Ok(None)` si absent ou vide.
pub fn load_session_cache_from_path(path: &Path) -> Result<Option<SessionCache>, String> {
    let _guard = CACHE_IO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if !path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)
        .map_err(|err| format!("Impossible de lire {} : {err}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let parsed: SessionCache = serde_json::from_str(&raw)
        .map_err(|err| format!("Fichier cache.json corrompu ou invalide : {err}"))?;

    if parsed.tracks.is_empty() {
        return Ok(None);
    }

    Ok(Some(sanitize_session(parsed)))
}

fn clear_session_cache_at_path_unlocked(path: &Path) -> Result<(), String> {
    if path.is_file() {
        fs::remove_file(path)
            .map_err(|err| format!("Impossible de supprimer {} : {err}", path.display()))?;
    }
    let tmp_path = path.with_extension("json.tmp");
    if tmp_path.is_file() {
        let _ = fs::remove_file(&tmp_path);
    }
    Ok(())
}

/// Supprime `cache.json` lorsqu'une nouvelle session démarre ou que la file est vidée.
pub fn clear_session_cache_at_path(path: &Path) -> Result<(), String> {
    let _guard = CACHE_IO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_session_cache_at_path_unlocked(path)
}

/// Résume une session pour la bannière/modale de reprise au démarrage.
pub fn summarize_session(
    session: Option<&SessionCache>,
    cache_file_path: &Path,
) -> SessionCacheSummary {
    let path_str = cache_file_path.to_string_lossy().into_owned();
    let Some(session) = session else {
        return SessionCacheSummary {
            has_session: false,
            is_unfinished: false,
            updated_at_ms: 0,
            export_mode: ExportMode::Dynamic,
            total_tracks: 0,
            exported_count: 0,
            identified_count: 0,
            needs_review_count: 0,
            remaining_count: 0,
            cache_file_path: path_str,
        };
    };

    let total_tracks = session.tracks.len();
    let mut exported_count = 0usize;
    let mut identified_count = 0usize;
    let mut needs_review_count = 0usize;

    for track in &session.tracks {
        if track.status == TrackStatus::Completed && track.exported_path.is_some() {
            exported_count += 1;
        } else if track.status == TrackStatus::Completed {
            identified_count += 1;
        } else if matches!(track.status, TrackStatus::NeedsReview | TrackStatus::Failed) {
            needs_review_count += 1;
        }
    }

    let remaining_count = total_tracks.saturating_sub(exported_count);
    let has_session = total_tracks > 0;
    let is_unfinished = has_session && remaining_count > 0;

    SessionCacheSummary {
        has_session,
        is_unfinished,
        updated_at_ms: session.updated_at_ms,
        export_mode: session.export_mode.clone(),
        total_tracks,
        exported_count,
        identified_count,
        needs_review_count,
        remaining_count,
        cache_file_path: path_str,
    }
}

pub fn save_session_cache(app: &tauri::AppHandle, session: &SessionCache) -> Result<(), String> {
    let path = resolve_cache_file_path(Some(app));
    save_session_cache_to_path(&path, session)
}

pub fn load_session_cache(app: &tauri::AppHandle) -> Result<Option<SessionCache>, String> {
    let path = resolve_cache_file_path(Some(app));
    load_session_cache_from_path(&path)
}

pub fn get_session_cache_summary(app: &tauri::AppHandle) -> Result<SessionCacheSummary, String> {
    let path = resolve_cache_file_path(Some(app));
    let session = load_session_cache_from_path(&path).unwrap_or_default();
    Ok(summarize_session(session.as_ref(), &path))
}

pub fn clear_session_cache(app: &tauri::AppHandle) -> Result<(), String> {
    let path = resolve_cache_file_path(Some(app));
    clear_session_cache_at_path(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AudioTrackItem, MetadataSource, TrackMetadata};

    fn temp_cache_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock error")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("tagmaster_dj_cache_{label}_{nanos}"))
            .join(CACHE_FILENAME)
    }

    fn make_track(
        id: &str,
        status: TrackStatus,
        exported_path: Option<&str>,
        title: Option<&str>,
    ) -> AudioTrackItem {
        AudioTrackItem {
            id: id.to_string(),
            source_path: format!("/music/{id}.mp3"),
            file_name: format!("{id}.mp3"),
            parent_folder: "Set Club".to_string(),
            extension: "MP3".to_string(),
            size_bytes: 5_242_880,
            has_protected_keyword: false,
            status,
            metadata: title.map(|t| TrackMetadata {
                title: t.to_string(),
                artist: "Daft Punk".to_string(),
                album_artist: "Daft Punk".to_string(),
                album: "Random Access Memories".to_string(),
                track_number: Some(8),
                track_total: Some(13),
                disc_number: Some(1),
                disc_total: Some(1),
                year: Some(2013),
                genre: Some("Electronic".to_string()),
                artwork_url: Some("https://example.com/cover.jpg".to_string()),
                is_compilation: false,
                source: Some(MetadataSource::AppleMusic),
                lyrics: None,
                synced_lyrics: None,
            }),
            exported_path: exported_path.map(str::to_string),
            local_artwork_path: None,
            embedded_artwork_path: None,
            review_reason: None,
            candidates: Vec::new(),
            last_query_used: None,
            duration_secs: Some(248),
            bitrate_kbps: Some(320),
            sample_rate_hz: Some(44_100),
        }
    }

    #[test]
    fn saves_loads_and_sanitizes_session_cache_roundtrip() {
        let path = temp_cache_path("roundtrip");
        let session = SessionCache {
            version: SESSION_CACHE_VERSION,
            updated_at_ms: 0,
            export_mode: ExportMode::Flat,
            tracks: vec![
                make_track(
                    "track_1",
                    TrackStatus::Completed,
                    Some("/export/Daft Punk - Get Lucky.mp3"),
                    Some("Get Lucky"),
                ),
                make_track("track_2", TrackStatus::Processing, None, None),
                make_track("track_3", TrackStatus::NeedsReview, None, None),
            ],
        };

        save_session_cache_to_path(&path, &session).expect("save should succeed");
        assert!(path.is_file(), "cache.json must exist on disk");

        let loaded = load_session_cache_from_path(&path)
            .expect("load should succeed")
            .expect("session should be present");
        assert_eq!(loaded.export_mode, ExportMode::Flat);
        assert_eq!(loaded.tracks.len(), 3);
        assert_eq!(
            loaded.tracks[1].status,
            TrackStatus::Pending,
            "transient Processing status must be restored as Pending"
        );
        assert!(loaded.updated_at_ms > 0);

        let summary = summarize_session(Some(&loaded), &path);
        assert!(summary.has_session);
        assert!(summary.is_unfinished);
        assert_eq!(summary.total_tracks, 3);
        assert_eq!(summary.exported_count, 1);
        assert_eq!(summary.needs_review_count, 1);
        assert_eq!(summary.remaining_count, 2);

        clear_session_cache_at_path(&path).expect("clear should succeed");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn detects_completed_session_as_not_unfinished_and_clears_on_empty_save() {
        let path = temp_cache_path("finished");
        let finished_session = SessionCache {
            version: SESSION_CACHE_VERSION,
            updated_at_ms: 1_700_000_000_000,
            export_mode: ExportMode::Dynamic,
            tracks: vec![make_track(
                "track_done",
                TrackStatus::Completed,
                Some("/export/done.mp3"),
                Some("Instant Crush"),
            )],
        };

        save_session_cache_to_path(&path, &finished_session).expect("save should succeed");
        let summary = summarize_session(Some(&finished_session), &path);
        assert!(summary.has_session);
        assert!(!summary.is_unfinished);
        assert_eq!(summary.remaining_count, 0);

        let empty_session = SessionCache {
            version: SESSION_CACHE_VERSION,
            updated_at_ms: 0,
            export_mode: ExportMode::Dynamic,
            tracks: Vec::new(),
        };
        save_session_cache_to_path(&path, &empty_session).expect("empty save clears file");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}

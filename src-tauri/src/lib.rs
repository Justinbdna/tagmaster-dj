pub mod api;
pub mod cache;
pub mod duplicates;
pub mod exporter;
pub mod models;
pub mod scanner;
pub mod tagger;

use std::path::PathBuf;

use models::{
    ApiKeysSettings, ArtworkCandidate, AudioTrackItem, BatchSummary, BatchTrackInput,
    DuplicateGroup, ExistingTrackTags, ExportMode, HealthStatus, LookupStatus, ManualExportInput,
    ManualExportResult, MetadataLookupResult, MetadataSource, PipelineProgressEvent, ScanResult,
    SessionCache, SessionCacheSummary, TrackCandidate, TrackStatus,
};
use tagger::rules::{RawMetadataInput, normalize_track_metadata};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

pub const PIPELINE_PROGRESS_EVENT: &str = "pipeline://progress";
const NETWORK_FAILURE_THRESHOLD: usize = 2;

/// Détecte si une erreur API correspond à une coupure réseau, un timeout ou un rate-limit HTTP.
pub fn is_network_or_rate_limit_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    const PATTERNS: &[&str] = &[
        "erreur réseau",
        "http 429",
        "429 too many",
        "too many requests",
        "http 500",
        "http 502",
        "http 503",
        "http 504",
        "rate limit",
        "rate-limit",
        "timed out",
        "timeout",
        "dns",
        "failed to lookup",
        "connection",
        "network",
        "unreachable",
        "reset by peer",
        "error sending request",
    ];
    PATTERNS.iter().any(|p| lower.contains(p))
}

fn resolve_export_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let documents_dir = app
        .path()
        .document_dir()
        .map_err(|err| format!("Impossible de résoudre le dossier Documents : {err}"))?;
    Ok(documents_dir.join(exporter::DEFAULT_EXPORT_DIR_NAME))
}

#[tauri::command]
fn health_check() -> HealthStatus {
    HealthStatus {
        app_name: "Tagmaster DJ".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        status: "ok".to_string(),
    }
}

#[tauri::command]
async fn scan_audio_paths(paths: Vec<String>) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scanner::scan_audio_paths(&paths))
        .await
        .map_err(|err| format!("Erreur interne du scanner audio : {err}"))?
}

#[tauri::command]
async fn read_track_tags(source_path: String) -> Result<ExistingTrackTags, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = PathBuf::from(&source_path);
        tagger::read_existing_tags(&path)
    })
    .await
    .map_err(|err| format!("Erreur interne de lecture des tags : {err}"))?
}

#[tauri::command]
async fn lookup_track_metadata(source_path: String) -> Result<MetadataLookupResult, String> {
    let path = PathBuf::from(&source_path);
    let path_for_read = path.clone();
    let existing = tauri::async_runtime::spawn_blocking(move || {
        tagger::read_existing_tags(&path_for_read).unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    api::resolve_track_metadata(&path, &existing).await
}

#[tauri::command]
async fn open_export_directory(app: tauri::AppHandle) -> Result<String, String> {
    let export_root = resolve_export_root(&app)?;
    std::fs::create_dir_all(&export_root).map_err(|err| {
        format!(
            "Impossible de préparer le dossier d'export {} : {err}",
            export_root.display()
        )
    })?;

    let export_str = export_root.to_string_lossy().into_owned();
    app.opener()
        .open_path(export_str.clone(), None::<&str>)
        .map_err(|err| format!("Impossible d'ouvrir le dossier d'export : {err}"))?;

    Ok(export_str)
}

#[tauri::command]
async fn run_batch_pipeline(
    app: tauri::AppHandle,
    tracks: Vec<BatchTrackInput>,
    export_mode: ExportMode,
) -> Result<BatchSummary, String> {
    let export_root = resolve_export_root(&app)?;
    std::fs::create_dir_all(&export_root).map_err(|err| {
        format!(
            "Impossible de créer le dossier d'export {} : {err}",
            export_root.display()
        )
    })?;

    let total = tracks.len();
    let mut completed_count: usize = 0;
    let mut needs_review_count: usize = 0;
    let mut failed_count: usize = 0;
    let mut consecutive_network_errors: usize = 0;
    let mut interrupted_by_network = false;
    let mut network_error_message: Option<String> = None;
    let mut remaining_count: usize = 0;

    for (idx, item) in tracks.into_iter().enumerate() {
        let current_index = idx + 1;
        let source_path = PathBuf::from(&item.source_path);
        let current_file_name = source_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| item.source_path.clone());

        let start_percent = ((idx * 100) / total.max(1)) as u8;
        let _ = app.emit(
            PIPELINE_PROGRESS_EVENT,
            &PipelineProgressEvent {
                track_id: item.id.clone(),
                current_index,
                total_tracks: total,
                percent: start_percent,
                current_file_name: current_file_name.clone(),
                track_status: TrackStatus::Processing,
                metadata: item.cached_metadata.clone(),
                exported_path: None,
                review_reason: None,
                query_used: None,
                candidates: Vec::new(),
            },
        );

        let done_percent = ((current_index * 100) / total.max(1)) as u8;

        // 1. Résolution des métadonnées (cache UI si déjà identifié, sinon Apple Music -> fallback yt-dlp)
        let mut metadata = if let Some(cached) = item.cached_metadata {
            cached
        } else {
            let path_for_read = source_path.clone();
            let existing = tauri::async_runtime::spawn_blocking(move || {
                tagger::read_existing_tags(&path_for_read).unwrap_or_default()
            })
            .await
            .unwrap_or_default();

            match api::resolve_track_metadata(&source_path, &existing).await {
                Ok(lookup)
                    if lookup.status == LookupStatus::Matched && lookup.metadata.is_some() =>
                {
                    consecutive_network_errors = 0;
                    lookup.metadata.unwrap()
                }
                Ok(lookup) => {
                    consecutive_network_errors = 0;
                    needs_review_count += 1;
                    let _ = app.emit(
                        PIPELINE_PROGRESS_EVENT,
                        &PipelineProgressEvent {
                            track_id: item.id.clone(),
                            current_index,
                            total_tracks: total,
                            percent: done_percent,
                            current_file_name,
                            track_status: TrackStatus::NeedsReview,
                            metadata: None,
                            exported_path: None,
                            review_reason: lookup.reason,
                            query_used: Some(lookup.query_used),
                            candidates: lookup.candidates,
                        },
                    );
                    continue;
                }
                Err(err) => {
                    failed_count += 1;
                    let is_net_err = is_network_or_rate_limit_error(&err);
                    let _ = app.emit(
                        PIPELINE_PROGRESS_EVENT,
                        &PipelineProgressEvent {
                            track_id: item.id.clone(),
                            current_index,
                            total_tracks: total,
                            percent: done_percent,
                            current_file_name,
                            track_status: TrackStatus::Failed,
                            metadata: None,
                            exported_path: None,
                            review_reason: Some(err.clone()),
                            query_used: None,
                            candidates: Vec::new(),
                        },
                    );
                    if is_net_err {
                        consecutive_network_errors += 1;
                        if consecutive_network_errors >= NETWORK_FAILURE_THRESHOLD
                            || current_index == total
                        {
                            interrupted_by_network = true;
                            network_error_message = Some(err);
                            remaining_count =
                                (total.saturating_sub(current_index)) + consecutive_network_errors;
                            break;
                        }
                    } else {
                        consecutive_network_errors = 0;
                    }
                    continue;
                }
            }
        };

        // 2. Paroles synchronisées temps réel (LRCLIB sans clé -> SYLT + .lrc) + fallback Genius (.env)
        if metadata.lyrics.is_none() || metadata.synced_lyrics.is_none() {
            let file_name_hint = source_path.file_name().and_then(|n| n.to_str());
            let resolved_lyrics = api::genius::resolve_lyrics_for_track(
                &metadata.artist,
                &metadata.title,
                Some(&metadata.album),
                file_name_hint,
                Some(&app),
            )
            .await;
            if metadata.lyrics.is_none() {
                metadata.lyrics = resolved_lyrics.plain_lyrics;
            }
            if metadata.synced_lyrics.is_none() {
                metadata.synced_lyrics = resolved_lyrics.synced_lyrics;
            }
        }

        // 3. Téléchargement tolérant aux pannes (ou lecture pochette locale validée en révision)
        let artwork = if let Some(local_art_path) = item
            .local_artwork_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
        {
            tauri::async_runtime::spawn_blocking(move || tagger::load_local_artwork(&local_art_path))
                .await
                .ok()
                .and_then(|res| res.ok())
        } else if let Some(url) = metadata.artwork_url.as_deref() {
            api::apple_music::download_artwork(url).await.ok()
        } else {
            None
        };

        // 3. Export non destructif + Table Rase + Écriture tags ID3/MP4/FLAC
        let source_for_export = source_path.clone();
        let export_root_clone = export_root.clone();
        let mode_clone = export_mode.clone();
        let meta_clone = metadata.clone();

        let export_result = tauri::async_runtime::spawn_blocking(move || {
            exporter::export_single_track(
                &source_for_export,
                &export_root_clone,
                mode_clone,
                &meta_clone,
                artwork.as_ref(),
            )
        })
        .await
        .map_err(|err| format!("Erreur interne d'export : {err}"))
        .and_then(|inner| inner);

        match export_result {
            Ok(exported_path) => {
                completed_count += 1;
                let _ = app.emit(
                    PIPELINE_PROGRESS_EVENT,
                    &PipelineProgressEvent {
                        track_id: item.id,
                        current_index,
                        total_tracks: total,
                        percent: done_percent,
                        current_file_name,
                        track_status: TrackStatus::Completed,
                        metadata: Some(metadata),
                        exported_path: Some(exported_path.to_string_lossy().into_owned()),
                        review_reason: None,
                        query_used: None,
                        candidates: Vec::new(),
                    },
                );
            }
            Err(err) => {
                failed_count += 1;
                let _ = app.emit(
                    PIPELINE_PROGRESS_EVENT,
                    &PipelineProgressEvent {
                        track_id: item.id,
                        current_index,
                        total_tracks: total,
                        percent: done_percent,
                        current_file_name,
                        track_status: TrackStatus::Failed,
                        metadata: Some(metadata),
                        exported_path: None,
                        review_reason: Some(err),
                        query_used: None,
                        candidates: Vec::new(),
                    },
                );
            }
        }
    }

    Ok(BatchSummary {
        total,
        completed_count,
        needs_review_count,
        failed_count,
        export_directory: export_root.to_string_lossy().into_owned(),
        interrupted_by_network,
        network_error_message,
        remaining_count,
    })
}

#[tauri::command]
async fn search_apple_music_candidates(query: String) -> Result<Vec<TrackCandidate>, String> {
    api::apple_music::search_candidates_freeform(&query).await
}

#[tauri::command]
async fn search_ytdlp_candidates(query: String) -> Result<Vec<TrackCandidate>, String> {
    api::ytdlp::search_candidates(&query)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn search_artwork_candidates(query: String) -> Result<Vec<ArtworkCandidate>, String> {
    api::apple_music::search_artwork_candidates(&query).await
}

#[tauri::command]
fn get_api_keys_settings(app: tauri::AppHandle) -> Result<ApiKeysSettings, String> {
    Ok(api::genius::get_api_keys_settings(&app))
}

#[tauri::command]
fn save_genius_api_key(
    app: tauri::AppHandle,
    api_key: String,
) -> Result<ApiKeysSettings, String> {
    api::genius::save_genius_api_key(&app, &api_key)
}

#[tauri::command]
async fn export_manual_track(
    app: tauri::AppHandle,
    input: ManualExportInput,
) -> Result<ManualExportResult, String> {
    let title = input.title.trim().to_string();
    let artist = input.artist.trim().to_string();
    if title.is_empty() || artist.is_empty() {
        return Err("Le titre et l'artiste sont obligatoires pour exporter.".to_string());
    }

    let export_root = resolve_export_root(&app)?;
    std::fs::create_dir_all(&export_root).map_err(|err| {
        format!(
            "Impossible de créer le dossier d'export {} : {err}",
            export_root.display()
        )
    })?;

    let mut metadata = normalize_track_metadata(RawMetadataInput {
        title,
        artist,
        collection_artist: input
            .album_artist
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        album: input
            .album
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        track_number: input.track_number,
        track_total: input.track_total,
        disc_number: input.disc_number,
        disc_total: input.disc_total,
        year: input.year,
        genre: input
            .genre
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        artwork_url: input
            .artwork_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        is_compilation: input.is_compilation.unwrap_or(false),
        source: input.source.unwrap_or(MetadataSource::Manual),
    });

    let artwork = if let Some(local_path) = input
        .local_artwork_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
    {
        let loaded = tauri::async_runtime::spawn_blocking(move || tagger::load_local_artwork(&local_path))
            .await
            .map_err(|err| format!("Erreur interne de lecture pochette : {err}"))??;
        Some(loaded)
    } else if let Some(url) = metadata.artwork_url.as_deref() {
        api::apple_music::download_artwork(url).await.ok()
    } else {
        None
    };

    let source_path = PathBuf::from(&input.source_path);
    if metadata.lyrics.is_none() || metadata.synced_lyrics.is_none() {
        let file_name_hint = source_path.file_name().and_then(|n| n.to_str());
        let resolved_lyrics = api::genius::resolve_lyrics_for_track(
            &metadata.artist,
            &metadata.title,
            Some(&metadata.album),
            file_name_hint,
            Some(&app),
        )
        .await;
        if metadata.lyrics.is_none() {
            metadata.lyrics = resolved_lyrics.plain_lyrics;
        }
        if metadata.synced_lyrics.is_none() {
            metadata.synced_lyrics = resolved_lyrics.synced_lyrics;
        }
    }

    let meta_clone = metadata.clone();
    let exported_path = tauri::async_runtime::spawn_blocking(move || {
        exporter::export_single_track(
            &source_path,
            &export_root,
            input.export_mode,
            &meta_clone,
            artwork.as_ref(),
        )
    })
    .await
    .map_err(|err| format!("Erreur interne d'export : {err}"))??;

    Ok(ManualExportResult {
        metadata,
        exported_path: exported_path.to_string_lossy().into_owned(),
    })
}

/// Identification Apple Music en lot en lecture seule (aucun fichier écrit/exporté).
#[tauri::command]
async fn run_batch_lookup(
    app: tauri::AppHandle,
    tracks: Vec<BatchTrackInput>,
) -> Result<BatchSummary, String> {
    let total = tracks.len();
    let mut completed_count: usize = 0;
    let mut needs_review_count: usize = 0;
    let mut failed_count: usize = 0;
    let mut consecutive_network_errors: usize = 0;
    let mut interrupted_by_network = false;
    let mut network_error_message: Option<String> = None;
    let mut remaining_count: usize = 0;

    for (idx, item) in tracks.into_iter().enumerate() {
        let current_index = idx + 1;
        let source_path = PathBuf::from(&item.source_path);
        let current_file_name = source_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| item.source_path.clone());

        let start_percent = ((idx * 100) / total.max(1)) as u8;
        let _ = app.emit(
            PIPELINE_PROGRESS_EVENT,
            &PipelineProgressEvent {
                track_id: item.id.clone(),
                current_index,
                total_tracks: total,
                percent: start_percent,
                current_file_name: current_file_name.clone(),
                track_status: TrackStatus::Processing,
                metadata: item.cached_metadata.clone(),
                exported_path: None,
                review_reason: None,
                query_used: None,
                candidates: Vec::new(),
            },
        );

        let done_percent = ((current_index * 100) / total.max(1)) as u8;
        let path_for_read = source_path.clone();
        let existing = tauri::async_runtime::spawn_blocking(move || {
            tagger::read_existing_tags(&path_for_read).unwrap_or_default()
        })
        .await
        .unwrap_or_default();

        match api::resolve_track_metadata(&source_path, &existing).await {
            Ok(lookup)
                if lookup.status == LookupStatus::Matched && lookup.metadata.is_some() =>
            {
                consecutive_network_errors = 0;
                completed_count += 1;
                let _ = app.emit(
                    PIPELINE_PROGRESS_EVENT,
                    &PipelineProgressEvent {
                        track_id: item.id,
                        current_index,
                        total_tracks: total,
                        percent: done_percent,
                        current_file_name,
                        track_status: TrackStatus::Completed,
                        metadata: lookup.metadata,
                        exported_path: None,
                        review_reason: None,
                        query_used: Some(lookup.query_used),
                        candidates: Vec::new(),
                    },
                );
            }
            Ok(lookup) => {
                consecutive_network_errors = 0;
                needs_review_count += 1;
                let _ = app.emit(
                    PIPELINE_PROGRESS_EVENT,
                    &PipelineProgressEvent {
                        track_id: item.id,
                        current_index,
                        total_tracks: total,
                        percent: done_percent,
                        current_file_name,
                        track_status: TrackStatus::NeedsReview,
                        metadata: None,
                        exported_path: None,
                        review_reason: lookup.reason,
                        query_used: Some(lookup.query_used),
                        candidates: lookup.candidates,
                    },
                );
            }
            Err(err) => {
                failed_count += 1;
                let is_net_err = is_network_or_rate_limit_error(&err);
                let _ = app.emit(
                    PIPELINE_PROGRESS_EVENT,
                    &PipelineProgressEvent {
                        track_id: item.id,
                        current_index,
                        total_tracks: total,
                        percent: done_percent,
                        current_file_name,
                        track_status: TrackStatus::Failed,
                        metadata: None,
                        exported_path: None,
                        review_reason: Some(err.clone()),
                        query_used: None,
                        candidates: Vec::new(),
                    },
                );
                if is_net_err {
                    consecutive_network_errors += 1;
                    if consecutive_network_errors >= NETWORK_FAILURE_THRESHOLD
                        || current_index == total
                    {
                        interrupted_by_network = true;
                        network_error_message = Some(err);
                        remaining_count =
                            (total.saturating_sub(current_index)) + consecutive_network_errors;
                        break;
                    }
                } else {
                    consecutive_network_errors = 0;
                }
            }
        }
    }

    Ok(BatchSummary {
        total,
        completed_count,
        needs_review_count,
        failed_count,
        export_directory: String::new(),
        interrupted_by_network,
        network_error_message,
        remaining_count,
    })
}

#[tauri::command]
fn get_session_cache_summary(app: tauri::AppHandle) -> Result<SessionCacheSummary, String> {
    cache::get_session_cache_summary(&app)
}

#[tauri::command]
fn load_session_cache(app: tauri::AppHandle) -> Result<Option<SessionCache>, String> {
    cache::load_session_cache(&app)
}

#[tauri::command]
fn save_session_cache(app: tauri::AppHandle, session: SessionCache) -> Result<(), String> {
    cache::save_session_cache(&app, &session)
}

#[tauri::command]
fn clear_session_cache(app: tauri::AppHandle) -> Result<(), String> {
    cache::clear_session_cache(&app)
}

#[tauri::command]
fn detect_duplicate_groups(tracks: Vec<AudioTrackItem>) -> Vec<DuplicateGroup> {
    duplicates::detect_duplicate_groups(&tracks)
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            scan_audio_paths,
            read_track_tags,
            lookup_track_metadata,
            open_export_directory,
            run_batch_lookup,
            run_batch_pipeline,
            search_apple_music_candidates,
            search_ytdlp_candidates,
            search_artwork_candidates,
            get_api_keys_settings,
            save_genius_api_key,
            export_manual_track,
            get_session_cache_summary,
            load_session_cache,
            save_session_cache,
            clear_session_cache,
            detect_duplicate_groups,
            exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::is_network_or_rate_limit_error;

    #[test]
    fn classifies_network_and_rate_limit_errors_accurately() {
        assert!(is_network_or_rate_limit_error(
            "Erreur réseau Apple Music (US) : error sending request"
        ));
        assert!(is_network_or_rate_limit_error(
            "API Apple Music (US) a répondu avec le statut HTTP 429 Too Many Requests"
        ));
        assert!(is_network_or_rate_limit_error(
            "API Apple Music (GB) a répondu avec le statut HTTP 503"
        ));
        assert!(is_network_or_rate_limit_error("request timed out"));
        assert!(!is_network_or_rate_limit_error(
            "Format audio non reconnu pour demo.mp3"
        ));
    }
}

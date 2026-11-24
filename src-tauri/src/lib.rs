pub mod api;
pub mod cache;
pub mod exporter;
pub mod models;
pub mod scanner;
pub mod tagger;

use std::path::PathBuf;

use models::{
    BatchSummary, BatchTrackInput, ExistingTrackTags, ExportMode, HealthStatus, LookupStatus,
    MetadataLookupResult, PipelineProgressEvent, ScanResult, TrackStatus,
};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

pub const PIPELINE_PROGRESS_EVENT: &str = "pipeline://progress";

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

    api::apple_music::search_track(&path, &existing).await
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
            },
        );

        let done_percent = ((current_index * 100) / total.max(1)) as u8;

        // 1. Résolution des métadonnées (cache UI si déjà prévisualisé, sinon Apple Music US/UK/FR)
        let metadata = if let Some(cached) = item.cached_metadata {
            cached
        } else {
            let path_for_read = source_path.clone();
            let existing = tauri::async_runtime::spawn_blocking(move || {
                tagger::read_existing_tags(&path_for_read).unwrap_or_default()
            })
            .await
            .unwrap_or_default();

            match api::apple_music::search_track(&source_path, &existing).await {
                Ok(lookup)
                    if lookup.status == LookupStatus::Matched && lookup.metadata.is_some() =>
                {
                    lookup.metadata.unwrap()
                }
                Ok(lookup) => {
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
                        },
                    );
                    continue;
                }
                Err(err) => {
                    failed_count += 1;
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
                            review_reason: Some(err),
                        },
                    );
                    continue;
                }
            }
        };

        // 2. Téléchargement tolérant aux pannes de la pochette HD
        let artwork = match metadata.artwork_url.as_deref() {
            Some(url) => api::apple_music::download_artwork(url).await.ok(),
            None => None,
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
    })
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
            run_batch_pipeline
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

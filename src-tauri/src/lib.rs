pub mod api;
pub mod cache;
pub mod exporter;
pub mod models;
pub mod scanner;
pub mod tagger;

use std::path::PathBuf;

use models::{ExistingTrackTags, HealthStatus, MetadataLookupResult, ScanResult};
use tauri::Manager;

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
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source_path.clone());

    let path_for_read = path.clone();
    let existing = tauri::async_runtime::spawn_blocking(move || {
        tagger::read_existing_tags(&path_for_read).unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    api::apple_music::search_track(
        &file_name,
        existing.artist.as_deref(),
        existing.title.as_deref(),
    )
    .await
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
            lookup_track_metadata
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

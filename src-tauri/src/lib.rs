pub mod api;
pub mod cache;
pub mod exporter;
pub mod models;
pub mod scanner;
pub mod tagger;

use models::{HealthStatus, ScanResult};
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
        .invoke_handler(tauri::generate_handler![health_check, scan_audio_paths])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

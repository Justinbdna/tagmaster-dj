pub mod api;
pub mod cache;
pub mod duplicates;
pub mod exporter;
pub mod ffmpeg;
pub mod models;
pub mod scanner;
pub mod tagger;

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use models::{
    ApiKeysSettings, ArtworkCandidate, ArtworkExtractInput, AudioTrackItem, BatchSummary,
    BatchTrackInput, BinaryDependencyStatus, DuplicateGroup, ExistingTrackTags, ExportFormatMode,
    ExportMode, FfmpegStatus, HardwareProfile, HealthStatus, LookupStatus, ManualExportInput,
    ManualExportResult, MetadataLookupResult, MetadataSource, PipelineProgressEvent,
    RuntimeDependenciesStatus, ScanResult, SearchSource, SessionCache, SessionCacheSummary,
    TrackCandidate, TrackEmbeddedArtwork, TrackStatus,
};
use tagger::rules::{RawMetadataInput, normalize_track_metadata};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex as AsyncMutex;

pub const PIPELINE_PROGRESS_EVENT: &str = "pipeline://progress";
pub const TRACK_ARTWORKS_EVENT: &str = "tracks://artworks";
pub const LOOKUP_CONCURRENCY_LIMIT: usize = 3;
const ARTWORK_BATCH_SIZE: usize = 24;
const NETWORK_FAILURE_THRESHOLD: usize = 2;

/// Compteur de génération pour invalider proprement les extractions de miniatures en cours
/// lorsque l'utilisateur vide la file ou démarre une nouvelle session.
static ARTWORK_EXTRACTION_GEN: AtomicU64 = AtomicU64::new(0);

/// Calcule le nombre de workers parallèles pour l'export (FFmpeg + tagging Table Rase)
/// selon les threads logiques disponibles, plafonné entre 2 et 4 pour ne jamais saturer le CPU.
pub fn compute_export_concurrency(logical_threads: usize) -> usize {
    (logical_threads / 2).clamp(2, 4)
}

fn current_export_concurrency() -> usize {
    let threads = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4);
    compute_export_concurrency(threads)
}

#[derive(Default)]
struct BatchSharedState {
    finished_count: usize,
    completed_count: usize,
    needs_review_count: usize,
    failed_count: usize,
    consecutive_network_errors: usize,
    interrupted_by_network: bool,
    network_error_message: Option<String>,
    last_exported_path: Option<String>,
}

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

fn resolve_export_root(
    app: &tauri::AppHandle,
    custom_export_dir: Option<&str>,
) -> Result<PathBuf, String> {
    let documents_dir = app
        .path()
        .document_dir()
        .map_err(|err| format!("Impossible de résoudre le dossier Documents : {err}"))?;
    Ok(exporter::resolve_export_root_path(
        &documents_dir,
        custom_export_dir,
    ))
}

#[tauri::command]
fn get_default_export_directory(app: tauri::AppHandle) -> Result<String, String> {
    let default_root = resolve_export_root(&app, None)?;
    Ok(default_root.to_string_lossy().into_owned())
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
async fn lookup_track_metadata(
    app: tauri::AppHandle,
    source_path: String,
) -> Result<MetadataLookupResult, String> {
    let path = PathBuf::from(&source_path);
    let path_for_read = path.clone();
    let existing = tauri::async_runtime::spawn_blocking(move || {
        tagger::read_existing_tags(&path_for_read).unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    api::resolve_track_metadata(&path, &existing, Some(&app)).await
}

#[tauri::command]
async fn open_export_directory(
    app: tauri::AppHandle,
    export_dir: Option<String>,
) -> Result<String, String> {
    let export_root = resolve_export_root(&app, export_dir.as_deref())?;
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
    format_mode: Option<ExportFormatMode>,
    hardware_profile: Option<HardwareProfile>,
    export_dir: Option<String>,
) -> Result<BatchSummary, String> {
    let format_mode = format_mode.unwrap_or_default();
    let hardware_profile = hardware_profile.unwrap_or_default();
    let export_root = resolve_export_root(&app, export_dir.as_deref())?;
    let export_root = exporter::validate_writable_export_dir(&export_root)?;

    let total = tracks.len();
    if total == 0 {
        return Ok(BatchSummary {
            total: 0,
            completed_count: 0,
            needs_review_count: 0,
            failed_count: 0,
            export_directory: export_root.to_string_lossy().into_owned(),
            interrupted_by_network: false,
            network_error_message: None,
            remaining_count: 0,
            last_exported_path: None,
        });
    }

    let worker_count = current_export_concurrency().min(total);
    let queue = Arc::new(AsyncMutex::new(VecDeque::from(tracks)));
    let state = Arc::new(AsyncMutex::new(BatchSharedState::default()));
    let stop_flag = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let app = app.clone();
        let queue = Arc::clone(&queue);
        let state = Arc::clone(&state);
        let stop_flag = Arc::clone(&stop_flag);
        let export_root = export_root.clone();
        let export_mode = export_mode.clone();
        let format_mode = format_mode.clone();
        let hardware_profile = hardware_profile.clone();

        handles.push(tauri::async_runtime::spawn(async move {
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                let item = {
                    let mut q = queue.lock().await;
                    q.pop_front()
                };
                let Some(item) = item else {
                    break;
                };

                let source_path = PathBuf::from(&item.source_path);
                let current_file_name = source_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| item.source_path.clone());

                let (start_index, start_percent) = {
                    let st = state.lock().await;
                    let idx = (st.finished_count + 1).min(total);
                    let pct = ((st.finished_count * 100) / total.max(1)) as u8;
                    (idx, pct)
                };

                let _ = app.emit(
                    PIPELINE_PROGRESS_EVENT,
                    &PipelineProgressEvent {
                        track_id: item.id.clone(),
                        current_index: start_index,
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

                // 1. Résolution des métadonnées (cache UI si déjà identifié, sinon Apple Music -> fallback)
                let mut metadata = if let Some(cached) = item.cached_metadata {
                    cached
                } else {
                    let path_for_read = source_path.clone();
                    let existing = tauri::async_runtime::spawn_blocking(move || {
                        tagger::read_existing_tags(&path_for_read).unwrap_or_default()
                    })
                    .await
                    .unwrap_or_default();

                    match api::resolve_track_metadata(&source_path, &existing, Some(&app)).await {
                        Ok(lookup) => match (lookup.status, lookup.metadata) {
                            (LookupStatus::Matched, Some(mut found_meta)) => {
                                if found_meta.bpm.is_none() {
                                    found_meta.bpm = existing.bpm;
                                }
                                let mut st = state.lock().await;
                                st.consecutive_network_errors = 0;
                                found_meta
                            }
                            (_, _) => {
                                let (done_index, done_percent) = {
                                    let mut st = state.lock().await;
                                    st.consecutive_network_errors = 0;
                                    st.finished_count += 1;
                                    st.needs_review_count += 1;
                                    let idx = st.finished_count;
                                    let pct = ((idx * 100) / total.max(1)) as u8;
                                    (idx, pct)
                                };
                                let _ = app.emit(
                                    PIPELINE_PROGRESS_EVENT,
                                    &PipelineProgressEvent {
                                        track_id: item.id.clone(),
                                        current_index: done_index,
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
                        },
                        Err(err) => {
                            let is_net_err = is_network_or_rate_limit_error(&err);
                            let (done_index, done_percent, should_stop) = {
                                let mut st = state.lock().await;
                                st.finished_count += 1;
                                st.failed_count += 1;
                                let idx = st.finished_count;
                                let pct = ((idx * 100) / total.max(1)) as u8;
                                let mut trip = false;
                                if is_net_err {
                                    st.consecutive_network_errors += 1;
                                    if st.consecutive_network_errors >= NETWORK_FAILURE_THRESHOLD
                                        || idx == total
                                    {
                                        st.interrupted_by_network = true;
                                        st.network_error_message = Some(err.clone());
                                        trip = true;
                                    }
                                } else {
                                    st.consecutive_network_errors = 0;
                                }
                                (idx, pct, trip)
                            };
                            let _ = app.emit(
                                PIPELINE_PROGRESS_EVENT,
                                &PipelineProgressEvent {
                                    track_id: item.id.clone(),
                                    current_index: done_index,
                                    total_tracks: total,
                                    percent: done_percent,
                                    current_file_name,
                                    track_status: TrackStatus::Failed,
                                    metadata: None,
                                    exported_path: None,
                                    review_reason: Some(err),
                                    query_used: None,
                                    candidates: Vec::new(),
                                },
                            );
                            if should_stop {
                                stop_flag.store(true, Ordering::SeqCst);
                                break;
                            }
                            continue;
                        }
                    }
                };

                // 2. Paroles synchronisées temps réel (LRCLIB sans clé -> SYLT + .lrc) + fallback Genius (.env)
                //    Ignoré pour le profil PSP (XMB ID3v2.3 strict sans .lrc ni SYLT).
                if hardware_profile != HardwareProfile::Psp
                    && (metadata.lyrics.is_none() || metadata.synced_lyrics.is_none())
                {
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
                    tauri::async_runtime::spawn_blocking(move || {
                        tagger::load_local_artwork(&local_art_path)
                    })
                    .await
                    .ok()
                    .and_then(|res| res.ok())
                } else if let Some(url) = metadata.artwork_url.as_deref() {
                    api::apple_music::download_artwork(url).await.ok()
                } else {
                    None
                };

                // 4. Export non destructif + FFmpeg (si requis) + Table Rase + Écriture tags selon profil
                let source_for_export = source_path.clone();
                let export_root_clone = export_root.clone();
                let mode_clone = export_mode.clone();
                let format_clone = format_mode.clone();
                let profile_clone = hardware_profile.clone();
                let meta_clone = metadata.clone();

                let export_result = tauri::async_runtime::spawn_blocking(move || {
                    exporter::export_single_track_with_profile(
                        &source_for_export,
                        &export_root_clone,
                        mode_clone,
                        format_clone,
                        profile_clone,
                        &meta_clone,
                        artwork.as_ref(),
                    )
                })
                .await
                .map_err(|err| format!("Erreur interne d'export : {err}"))
                .and_then(|inner| inner);

                match export_result {
                    Ok(exported_path) => {
                        let exported_str = exported_path.to_string_lossy().into_owned();
                        let (done_index, done_percent) = {
                            let mut st = state.lock().await;
                            st.finished_count += 1;
                            st.completed_count += 1;
                            st.last_exported_path = Some(exported_str.clone());
                            let idx = st.finished_count;
                            let pct = ((idx * 100) / total.max(1)) as u8;
                            (idx, pct)
                        };
                        let _ = app.emit(
                            PIPELINE_PROGRESS_EVENT,
                            &PipelineProgressEvent {
                                track_id: item.id,
                                current_index: done_index,
                                total_tracks: total,
                                percent: done_percent,
                                current_file_name,
                                track_status: TrackStatus::Completed,
                                metadata: Some(metadata),
                                exported_path: Some(exported_str),
                                review_reason: None,
                                query_used: None,
                                candidates: Vec::new(),
                            },
                        );
                    }
                    Err(err) => {
                        let (done_index, done_percent) = {
                            let mut st = state.lock().await;
                            st.finished_count += 1;
                            st.failed_count += 1;
                            let idx = st.finished_count;
                            let pct = ((idx * 100) / total.max(1)) as u8;
                            (idx, pct)
                        };
                        let _ = app.emit(
                            PIPELINE_PROGRESS_EVENT,
                            &PipelineProgressEvent {
                                track_id: item.id,
                                current_index: done_index,
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
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let unstarted = queue.lock().await.len();
    let st = state.lock().await;
    let remaining_count = if st.interrupted_by_network {
        unstarted + st.consecutive_network_errors
    } else {
        0
    };

    Ok(BatchSummary {
        total,
        completed_count: st.completed_count,
        needs_review_count: st.needs_review_count,
        failed_count: st.failed_count,
        export_directory: export_root.to_string_lossy().into_owned(),
        interrupted_by_network: st.interrupted_by_network,
        network_error_message: st.network_error_message.clone(),
        remaining_count,
        last_exported_path: st.last_exported_path.clone(),
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
async fn search_track_candidates(
    app: tauri::AppHandle,
    source: SearchSource,
    query: String,
) -> Result<Vec<TrackCandidate>, String> {
    api::search_candidates_by_source(source, &query, Some(&app)).await
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
fn save_discogs_token(
    app: tauri::AppHandle,
    token: String,
) -> Result<ApiKeysSettings, String> {
    api::genius::save_discogs_token(&app, &token)
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

    let export_root = resolve_export_root(&app, input.export_dir.as_deref())?;
    let export_root = exporter::validate_writable_export_dir(&export_root)?;

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
    metadata.bpm = input.bpm.filter(|&b| b > 0);

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
    if input.hardware_profile != HardwareProfile::Psp
        && (metadata.lyrics.is_none() || metadata.synced_lyrics.is_none())
    {
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
    let format_mode = input.format_mode;
    let hardware_profile = input.hardware_profile;
    let exported_path = tauri::async_runtime::spawn_blocking(move || {
        exporter::export_single_track_with_profile(
            &source_path,
            &export_root,
            input.export_mode,
            format_mode,
            hardware_profile,
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

/// Identification multi-sources en lot en lecture seule (pool borné à 4 pistes simultanées).
#[tauri::command]
async fn run_batch_lookup(
    app: tauri::AppHandle,
    tracks: Vec<BatchTrackInput>,
) -> Result<BatchSummary, String> {
    let total = tracks.len();
    if total == 0 {
        return Ok(BatchSummary {
            total: 0,
            completed_count: 0,
            needs_review_count: 0,
            failed_count: 0,
            export_directory: String::new(),
            interrupted_by_network: false,
            network_error_message: None,
            remaining_count: 0,
            last_exported_path: None,
        });
    }

    let worker_count = LOOKUP_CONCURRENCY_LIMIT.min(total);
    let queue = Arc::new(AsyncMutex::new(VecDeque::from(tracks)));
    let state = Arc::new(AsyncMutex::new(BatchSharedState::default()));
    let stop_flag = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let app = app.clone();
        let queue = Arc::clone(&queue);
        let state = Arc::clone(&state);
        let stop_flag = Arc::clone(&stop_flag);

        handles.push(tauri::async_runtime::spawn(async move {
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                let item = {
                    let mut q = queue.lock().await;
                    q.pop_front()
                };
                let Some(item) = item else {
                    break;
                };

                let source_path = PathBuf::from(&item.source_path);
                let current_file_name = source_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| item.source_path.clone());

                let (start_index, start_percent) = {
                    let st = state.lock().await;
                    let idx = (st.finished_count + 1).min(total);
                    let pct = ((st.finished_count * 100) / total.max(1)) as u8;
                    (idx, pct)
                };

                let _ = app.emit(
                    PIPELINE_PROGRESS_EVENT,
                    &PipelineProgressEvent {
                        track_id: item.id.clone(),
                        current_index: start_index,
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

                let path_for_read = source_path.clone();
                let existing = tauri::async_runtime::spawn_blocking(move || {
                    tagger::read_existing_tags(&path_for_read).unwrap_or_default()
                })
                .await
                .unwrap_or_default();

                match api::resolve_track_metadata(&source_path, &existing, Some(&app)).await {
                    Ok(lookup)
                        if lookup.status == LookupStatus::Matched && lookup.metadata.is_some() =>
                    {
                        let (done_index, done_percent) = {
                            let mut st = state.lock().await;
                            st.consecutive_network_errors = 0;
                            st.finished_count += 1;
                            st.completed_count += 1;
                            let idx = st.finished_count;
                            let pct = ((idx * 100) / total.max(1)) as u8;
                            (idx, pct)
                        };
                        let _ = app.emit(
                            PIPELINE_PROGRESS_EVENT,
                            &PipelineProgressEvent {
                                track_id: item.id,
                                current_index: done_index,
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
                        let (done_index, done_percent) = {
                            let mut st = state.lock().await;
                            st.consecutive_network_errors = 0;
                            st.finished_count += 1;
                            st.needs_review_count += 1;
                            let idx = st.finished_count;
                            let pct = ((idx * 100) / total.max(1)) as u8;
                            (idx, pct)
                        };
                        let _ = app.emit(
                            PIPELINE_PROGRESS_EVENT,
                            &PipelineProgressEvent {
                                track_id: item.id,
                                current_index: done_index,
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
                        let is_net_err = is_network_or_rate_limit_error(&err);
                        let (done_index, done_percent, should_stop) = {
                            let mut st = state.lock().await;
                            st.finished_count += 1;
                            st.failed_count += 1;
                            let idx = st.finished_count;
                            let pct = ((idx * 100) / total.max(1)) as u8;
                            let mut trip = false;
                            if is_net_err {
                                st.consecutive_network_errors += 1;
                                if st.consecutive_network_errors >= NETWORK_FAILURE_THRESHOLD
                                    || idx == total
                                {
                                    st.interrupted_by_network = true;
                                    st.network_error_message = Some(err.clone());
                                    trip = true;
                                }
                            } else {
                                st.consecutive_network_errors = 0;
                            }
                            (idx, pct, trip)
                        };
                        let _ = app.emit(
                            PIPELINE_PROGRESS_EVENT,
                            &PipelineProgressEvent {
                                track_id: item.id,
                                current_index: done_index,
                                total_tracks: total,
                                percent: done_percent,
                                current_file_name,
                                track_status: TrackStatus::Failed,
                                metadata: None,
                                exported_path: None,
                                review_reason: Some(err),
                                query_used: None,
                                candidates: Vec::new(),
                            },
                        );
                        if should_stop {
                            stop_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                    }
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    let unstarted = queue.lock().await.len();
    let st = state.lock().await;
    let remaining_count = if st.interrupted_by_network {
        unstarted + st.consecutive_network_errors
    } else {
        0
    };

    Ok(BatchSummary {
        total,
        completed_count: st.completed_count,
        needs_review_count: st.needs_review_count,
        failed_count: st.failed_count,
        export_directory: String::new(),
        interrupted_by_network: st.interrupted_by_network,
        network_error_message: st.network_error_message.clone(),
        remaining_count,
        last_exported_path: None,
    })
}

/// Extrait en arrière-plan les pochettes embarquées des pistes importées par lots
/// sans bloquer le scan ni saturer l'UI sur 3 000+ fichiers.
#[tauri::command]
fn extract_embedded_artworks(app: tauri::AppHandle, tracks: Vec<ArtworkExtractInput>) {
    if tracks.is_empty() {
        return;
    }

    let generation = ARTWORK_EXTRACTION_GEN.load(Ordering::SeqCst);

    tauri::async_runtime::spawn(async move {
        let worker_count = current_export_concurrency().min(tracks.len());
        let queue = Arc::new(AsyncMutex::new(VecDeque::from(tracks)));
        let batch_buf = Arc::new(AsyncMutex::new(Vec::<TrackEmbeddedArtwork>::with_capacity(
            ARTWORK_BATCH_SIZE,
        )));

        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let app = app.clone();
            let queue = Arc::clone(&queue);
            let batch_buf = Arc::clone(&batch_buf);

            handles.push(tauri::async_runtime::spawn(async move {
                loop {
                    if ARTWORK_EXTRACTION_GEN.load(Ordering::SeqCst) != generation {
                        break;
                    }

                    let item = {
                        let mut q = queue.lock().await;
                        q.pop_front()
                    };
                    let Some(item) = item else {
                        break;
                    };

                    let source_path = item.source_path;
                    let extracted = tauri::async_runtime::spawn_blocking(move || {
                        tagger::extract_embedded_artwork_fast(Path::new(&source_path))
                    })
                    .await
                    .ok()
                    .flatten();

                    if ARTWORK_EXTRACTION_GEN.load(Ordering::SeqCst) != generation {
                        if let Some(stale_path) = extracted {
                            let _ = tagger::remove_embedded_artwork_files(&[stale_path]);
                        }
                        break;
                    }

                    if let Some(embedded_artwork_path) = extracted {
                        let flush_batch = {
                            let mut buf = batch_buf.lock().await;
                            buf.push(TrackEmbeddedArtwork {
                                track_id: item.id,
                                embedded_artwork_path,
                            });
                            if buf.len() >= ARTWORK_BATCH_SIZE {
                                Some(std::mem::take(&mut *buf))
                            } else {
                                None
                            }
                        };
                        if let Some(items) = flush_batch
                            && ARTWORK_EXTRACTION_GEN.load(Ordering::SeqCst) == generation
                        {
                            let _ = app.emit(TRACK_ARTWORKS_EVENT, &items);
                        }
                    }
                }
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        if ARTWORK_EXTRACTION_GEN.load(Ordering::SeqCst) != generation {
            return;
        }

        let remaining = {
            let mut buf = batch_buf.lock().await;
            std::mem::take(&mut *buf)
        };
        if !remaining.is_empty() && ARTWORK_EXTRACTION_GEN.load(Ordering::SeqCst) == generation {
            let _ = app.emit(TRACK_ARTWORKS_EVENT, &remaining);
        }
    });
}

/// Annule toute extraction en cours et purge intégralement le dossier temporaire `tagmaster-dj-thumbs`.
#[tauri::command]
async fn clear_embedded_artwork_cache() -> Result<usize, String> {
    ARTWORK_EXTRACTION_GEN.fetch_add(1, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(tagger::clear_embedded_artwork_cache)
        .await
        .map_err(|err| format!("Erreur lors de la purge du cache de miniatures : {err}"))?
}

/// Supprime de façon ciblée les miniatures temporaires des pistes retirées.
#[tauri::command]
async fn remove_embedded_artwork_files(paths: Vec<String>) -> Result<usize, String> {
    if paths.is_empty() {
        return Ok(0);
    }
    tauri::async_runtime::spawn_blocking(move || tagger::remove_embedded_artwork_files(&paths))
        .await
        .map_err(|err| format!("Erreur lors du nettoyage des miniatures : {err}"))
}

/// Valide qu'un chemin fourni pour l'ouverture dans le Finder / Explorateur existe bien.
pub fn validate_reveal_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Chemin du fichier exporté vide.".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.exists() {
        return Err(format!(
            "Le fichier exporté est introuvable : {}",
            path.display()
        ));
    }
    Ok(path)
}

/// Révèle et sélectionne un fichier exporté dans le gestionnaire de fichiers natif.
#[tauri::command]
fn reveal_in_file_manager(path: String) -> Result<(), String> {
    let target = validate_reveal_path(&path)?;

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg("-R")
            .arg(&target)
            .status()
            .map_err(|err| format!("Impossible d'ouvrir le Finder : {err}"))?;
        if !status.success() {
            return Err(format!(
                "Le Finder n'a pas pu révéler le fichier : {}",
                target.display()
            ));
        }
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg("/select,")
            .arg(&target)
            .status()
            .map_err(|err| format!("Impossible d'ouvrir l'Explorateur Windows : {err}"))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = target.parent().unwrap_or(&target);
        let status = Command::new("xdg-open")
            .arg(dir)
            .status()
            .map_err(|err| format!("Impossible d'ouvrir le gestionnaire de fichiers : {err}"))?;
        if !status.success() {
            return Err(format!(
                "Impossible d'ouvrir le dossier parent : {}",
                dir.display()
            ));
        }
    }

    Ok(())
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
async fn save_session_cache(app: tauri::AppHandle, session: SessionCache) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || cache::save_session_cache(&app, &session))
        .await
        .map_err(|err| format!("Erreur interne de sauvegarde cache.json : {err}"))?
}

#[tauri::command]
async fn clear_session_cache(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || cache::clear_session_cache(&app))
        .await
        .map_err(|err| format!("Erreur interne de suppression cache.json : {err}"))?
}

#[tauri::command]
fn detect_duplicate_groups(tracks: Vec<AudioTrackItem>) -> Vec<DuplicateGroup> {
    duplicates::detect_duplicate_groups(&tracks)
}

/// Construit la commande d'installation adaptée à l'OS selon les dépendances manquantes.
pub fn build_dependency_install_command(
    platform: &str,
    missing_ffmpeg: bool,
    missing_ytdlp: bool,
) -> Option<String> {
    if !missing_ffmpeg && !missing_ytdlp {
        return None;
    }

    match platform {
        "macos" => {
            let mut pkgs = Vec::new();
            if missing_ffmpeg {
                pkgs.push("ffmpeg");
            }
            if missing_ytdlp {
                pkgs.push("yt-dlp");
            }
            Some(format!("brew install {}", pkgs.join(" ")))
        }
        "windows" => {
            let mut pkgs = Vec::new();
            if missing_ffmpeg {
                pkgs.push("Gyan.FFmpeg");
            }
            if missing_ytdlp {
                pkgs.push("yt-dlp.yt-dlp");
            }
            Some(format!("winget install {}", pkgs.join(" ")))
        }
        _ => {
            let mut pkgs = Vec::new();
            if missing_ffmpeg {
                pkgs.push("ffmpeg");
            }
            if missing_ytdlp {
                pkgs.push("yt-dlp");
            }
            Some(format!("sudo apt install {}", pkgs.join(" ")))
        }
    }
}

#[tauri::command]
fn check_ffmpeg_status() -> FfmpegStatus {
    ffmpeg::check_ffmpeg_status()
}

#[tauri::command]
async fn check_runtime_dependencies() -> Result<RuntimeDependenciesStatus, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let ffmpeg_raw = ffmpeg::check_ffmpeg_status();
        let ffmpeg_dep = BinaryDependencyStatus {
            name: "ffmpeg".to_string(),
            available: ffmpeg_raw.available,
            version: ffmpeg_raw.version,
            binary_path: ffmpeg_raw.binary_path,
        };
        let ytdlp_dep = api::ytdlp::check_ytdlp_status();

        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        };

        let all_available = ffmpeg_dep.available && ytdlp_dep.available;
        let install_command =
            build_dependency_install_command(platform, !ffmpeg_dep.available, !ytdlp_dep.available);

        RuntimeDependenciesStatus {
            platform: platform.to_string(),
            ffmpeg: ffmpeg_dep,
            ytdlp: ytdlp_dep,
            all_available,
            install_command,
        }
    })
    .await
    .map_err(|err| format!("Erreur lors du diagnostic des dépendances : {err}"))
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

            // Nettoyage automatique au démarrage des miniatures temporaires orphelines
            // en préservant uniquement celles référencées par une session cache.json active.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let mut active_thumbs: HashSet<PathBuf> = HashSet::new();
                if let Ok(Some(session)) = cache::load_session_cache(&app_handle) {
                    for track in session.tracks {
                        if let Some(thumb) = track.embedded_artwork_path {
                            active_thumbs.insert(PathBuf::from(thumb));
                        }
                    }
                }
                let _ = tagger::cleanup_orphan_embedded_artworks(&active_thumbs);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            check_ffmpeg_status,
            check_runtime_dependencies,
            scan_audio_paths,
            read_track_tags,
            lookup_track_metadata,
            get_default_export_directory,
            open_export_directory,
            reveal_in_file_manager,
            extract_embedded_artworks,
            clear_embedded_artwork_cache,
            remove_embedded_artwork_files,
            run_batch_lookup,
            run_batch_pipeline,
            search_apple_music_candidates,
            search_ytdlp_candidates,
            search_track_candidates,
            search_artwork_candidates,
            get_api_keys_settings,
            save_genius_api_key,
            save_discogs_token,
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
    use super::{
        LOOKUP_CONCURRENCY_LIMIT, build_dependency_install_command, compute_export_concurrency,
        is_network_or_rate_limit_error, validate_reveal_path,
    };

    #[test]
    fn lookup_concurrency_limit_is_balanced_at_three() {
        assert_eq!(LOOKUP_CONCURRENCY_LIMIT, 3);
    }

    #[test]
    fn clamps_export_concurrency_across_cpu_counts() {
        assert_eq!(compute_export_concurrency(1), 2);
        assert_eq!(compute_export_concurrency(2), 2);
        assert_eq!(compute_export_concurrency(3), 2);
        assert_eq!(compute_export_concurrency(4), 2);
        assert_eq!(compute_export_concurrency(6), 3);
        assert_eq!(compute_export_concurrency(8), 4);
        assert_eq!(compute_export_concurrency(12), 4);
        assert_eq!(compute_export_concurrency(16), 4);
    }

    #[test]
    fn validates_reveal_path_for_empty_missing_and_special_char_files() {
        assert!(validate_reveal_path("").is_err());
        assert!(validate_reveal_path("   ").is_err());
        assert!(validate_reveal_path("/nonexistent/tagmaster_dj_missing_file.mp3").is_err());

        let temp_file = std::env::temp_dir().join(format!(
            "tagmaster_dj_reveal_Été d'amour (Live Remix) {}.tmp",
            std::process::id()
        ));
        std::fs::write(&temp_file, b"ok").expect("create temp file for test");
        let validated = validate_reveal_path(temp_file.to_string_lossy().as_ref());
        let _ = std::fs::remove_file(&temp_file);
        assert!(validated.is_ok());
    }

    #[test]
    fn builds_os_specific_dependency_install_commands() {
        assert_eq!(
            build_dependency_install_command("macos", true, true).as_deref(),
            Some("brew install ffmpeg yt-dlp")
        );
        assert_eq!(
            build_dependency_install_command("macos", true, false).as_deref(),
            Some("brew install ffmpeg")
        );
        assert_eq!(
            build_dependency_install_command("windows", true, true).as_deref(),
            Some("winget install Gyan.FFmpeg yt-dlp.yt-dlp")
        );
        assert_eq!(
            build_dependency_install_command("windows", false, true).as_deref(),
            Some("winget install yt-dlp.yt-dlp")
        );
        assert_eq!(build_dependency_install_command("macos", false, false), None);
    }

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

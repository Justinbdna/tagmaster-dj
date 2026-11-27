import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ApiKeysSettings,
  ArtworkCandidate,
  AudioTrackItem,
  BatchSummary,
  BatchTrackInput,
  DuplicateGroup,
  ExistingTrackTags,
  ExportFormatMode,
  ExportMode,
  FfmpegStatus,
  HardwareProfile,
  HealthStatus,
  ManualExportInput,
  ManualExportResult,
  MetadataLookupResult,
  PipelineProgressEvent,
  QuickSearchSource,
  RuntimeDependenciesStatus,
  ScanResult,
  SessionCache,
  SessionCacheSummary,
  TrackCandidate,
} from "../types/track";

const AUDIO_EXTENSIONS = ["mp3", "m4a", "aac", "flac", "wav"];
const ARTWORK_EXTENSIONS = ["jpg", "jpeg", "png", "webp"];
const PIPELINE_PROGRESS_EVENT = "pipeline://progress";

export function isTauriEnvironment(): boolean {
  return isTauri();
}

export function toLocalAssetUrl(filePath: string): string {
  return isTauri() ? convertFileSrc(filePath) : filePath;
}

export async function checkBackendHealth(): Promise<HealthStatus> {
  return invoke<HealthStatus>("health_check");
}

export async function checkFfmpegStatus(): Promise<FfmpegStatus> {
  return invoke<FfmpegStatus>("check_ffmpeg_status");
}

export async function checkRuntimeDependencies(): Promise<RuntimeDependenciesStatus> {
  return invoke<RuntimeDependenciesStatus>("check_runtime_dependencies");
}

export async function scanAudioPaths(paths: string[]): Promise<ScanResult> {
  return invoke<ScanResult>("scan_audio_paths", { paths });
}

export async function readTrackTags(
  sourcePath: string,
): Promise<ExistingTrackTags> {
  return invoke<ExistingTrackTags>("read_track_tags", { sourcePath });
}

export async function lookupTrackMetadata(
  sourcePath: string,
): Promise<MetadataLookupResult> {
  return invoke<MetadataLookupResult>("lookup_track_metadata", { sourcePath });
}

export async function runBatchLookup(
  tracks: BatchTrackInput[],
): Promise<BatchSummary> {
  return invoke<BatchSummary>("run_batch_lookup", { tracks });
}

export async function runBatchPipeline(
  tracks: BatchTrackInput[],
  exportMode: ExportMode,
  formatMode?: ExportFormatMode,
  hardwareProfile?: HardwareProfile,
): Promise<BatchSummary> {
  return invoke<BatchSummary>("run_batch_pipeline", {
    tracks,
    exportMode,
    formatMode,
    hardwareProfile,
  });
}

export async function openExportDirectory(): Promise<string> {
  return invoke<string>("open_export_directory");
}

export async function searchAppleMusicCandidates(
  query: string,
): Promise<TrackCandidate[]> {
  return invoke<TrackCandidate[]>("search_apple_music_candidates", { query });
}

export async function searchYtdlpCandidates(
  query: string,
): Promise<TrackCandidate[]> {
  return invoke<TrackCandidate[]>("search_ytdlp_candidates", { query });
}

export async function searchTrackCandidates(
  source: QuickSearchSource,
  query: string,
): Promise<TrackCandidate[]> {
  return invoke<TrackCandidate[]>("search_track_candidates", { source, query });
}

export async function searchArtworkCandidates(
  query: string,
): Promise<ArtworkCandidate[]> {
  return invoke<ArtworkCandidate[]>("search_artwork_candidates", { query });
}

export async function getApiKeysSettings(): Promise<ApiKeysSettings> {
  return invoke<ApiKeysSettings>("get_api_keys_settings");
}

export async function saveGeniusApiKey(
  apiKey: string,
): Promise<ApiKeysSettings> {
  return invoke<ApiKeysSettings>("save_genius_api_key", { apiKey });
}

export async function saveDiscogsToken(
  token: string,
): Promise<ApiKeysSettings> {
  return invoke<ApiKeysSettings>("save_discogs_token", { token });
}

export async function exportManualTrack(
  input: ManualExportInput,
): Promise<ManualExportResult> {
  return invoke<ManualExportResult>("export_manual_track", { input });
}

export async function getSessionCacheSummary(): Promise<SessionCacheSummary> {
  return invoke<SessionCacheSummary>("get_session_cache_summary");
}

export async function loadSessionCache(): Promise<SessionCache | null> {
  return invoke<SessionCache | null>("load_session_cache");
}

export async function saveSessionCache(session: SessionCache): Promise<void> {
  return invoke<void>("save_session_cache", { session });
}

export async function clearSessionCache(): Promise<void> {
  return invoke<void>("clear_session_cache");
}

export async function detectDuplicateGroups(
  tracks: AudioTrackItem[],
): Promise<DuplicateGroup[]> {
  return invoke<DuplicateGroup[]>("detect_duplicate_groups", { tracks });
}

export async function exitApp(): Promise<void> {
  return invoke<void>("exit_app");
}

export async function openArtworkFileDialog(): Promise<string | null> {
  const result = await open({
    directory: false,
    multiple: false,
    title: "Choisir une pochette locale (JPG, PNG, WEBP)",
    filters: [
      {
        name: "Images (JPG, JPEG, PNG, WEBP)",
        extensions: ARTWORK_EXTENSIONS,
      },
    ],
  });
  const normalized = normalizeDialogResult(result);
  return normalized[0] ?? null;
}

export async function subscribeToPipelineProgress(
  onProgress: (event: PipelineProgressEvent) => void,
): Promise<() => void> {
  if (!isTauri()) {
    return () => {};
  }

  const unlisten = await listen<PipelineProgressEvent>(
    PIPELINE_PROGRESS_EVENT,
    (event) => {
      onProgress(event.payload);
    },
  );

  return unlisten;
}

function normalizeDialogResult(result: string | string[] | null): string[] {
  if (!result) return [];
  return Array.isArray(result) ? result : [result];
}

export async function openFolderDialog(): Promise<string[]> {
  const result = await open({
    directory: true,
    multiple: true,
    title: "Sélectionner un ou plusieurs dossiers musicaux",
  });
  return normalizeDialogResult(result);
}

export async function openAudioFilesDialog(): Promise<string[]> {
  const result = await open({
    directory: false,
    multiple: true,
    title: "Sélectionner des fichiers audio",
    filters: [
      {
        name: "Fichiers Audio (MP3, M4A, AAC, FLAC, WAV)",
        extensions: AUDIO_EXTENSIONS,
      },
    ],
  });
  return normalizeDialogResult(result);
}

export interface DragDropHandlers {
  onEnter: () => void;
  onLeave: () => void;
  onDrop: (paths: string[]) => void;
}

export async function subscribeToWindowDragDrop(
  handlers: DragDropHandlers,
): Promise<() => void> {
  if (!isTauri()) {
    return () => {};
  }

  const appWindow = getCurrentWindow();
  const unlisten = await appWindow.onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter" || payload.type === "over") {
      handlers.onEnter();
    } else if (payload.type === "drop") {
      handlers.onLeave();
      if (payload.paths.length > 0) {
        handlers.onDrop(payload.paths);
      }
    } else if (payload.type === "leave") {
      handlers.onLeave();
    }
  });

  return unlisten;
}

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BatchSummary,
  BatchTrackInput,
  ExistingTrackTags,
  ExportMode,
  HealthStatus,
  MetadataLookupResult,
  PipelineProgressEvent,
  ScanResult,
} from "../types/track";

const AUDIO_EXTENSIONS = ["mp3", "m4a", "aac", "flac", "wav"];
const PIPELINE_PROGRESS_EVENT = "pipeline://progress";

export function isTauriEnvironment(): boolean {
  return isTauri();
}

export async function checkBackendHealth(): Promise<HealthStatus> {
  return invoke<HealthStatus>("health_check");
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

export async function runBatchPipeline(
  tracks: BatchTrackInput[],
  exportMode: ExportMode,
): Promise<BatchSummary> {
  return invoke<BatchSummary>("run_batch_pipeline", { tracks, exportMode });
}

export async function openExportDirectory(): Promise<string> {
  return invoke<string>("open_export_directory");
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

import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type { HealthStatus, ScanResult } from "../types/track";

const AUDIO_EXTENSIONS = ["mp3", "m4a", "aac", "flac", "wav"];

export function isTauriEnvironment(): boolean {
  return isTauri();
}

export async function checkBackendHealth(): Promise<HealthStatus> {
  return invoke<HealthStatus>("health_check");
}

export async function scanAudioPaths(paths: string[]): Promise<ScanResult> {
  return invoke<ScanResult>("scan_audio_paths", { paths });
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

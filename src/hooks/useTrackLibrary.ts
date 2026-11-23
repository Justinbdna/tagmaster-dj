import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  isTauriEnvironment,
  openAudioFilesDialog,
  openFolderDialog,
  scanAudioPaths,
  subscribeToWindowDragDrop,
} from "../services/tauriBridge";
import type { AudioTrackItem } from "../types/track";
import { formatFileSize } from "../utils/format";

const PROTECTED_KEYWORDS = [
  "live",
  "acoustic",
  "sped up",
  "chopped",
  "instrumental",
  "remix",
  "mashup",
];

const SUPPORTED_EXT_SET = new Set(["MP3", "M4A", "AAC", "FLAC", "WAV"]);

function buildBrowserTrack(file: File): AudioTrackItem | null {
  const dotIdx = file.name.lastIndexOf(".");
  if (dotIdx === -1) return null;
  const ext = file.name.slice(dotIdx + 1).toUpperCase();
  if (!SUPPORTED_EXT_SET.has(ext)) return null;

  const lower = file.name.toLowerCase();
  const relPath =
    (file as File & { webkitRelativePath?: string }).webkitRelativePath ||
    file.name;
  const parts = relPath.split("/");
  const parentFolder =
    parts.length > 1 ? parts[parts.length - 2] : "Import navigateur";

  return {
    id: `${relPath.toLowerCase()}::${file.size}`,
    sourcePath: relPath,
    fileName: file.name,
    parentFolder,
    extension: ext,
    sizeBytes: file.size,
    hasProtectedKeyword: PROTECTED_KEYWORDS.some((kw) => lower.includes(kw)),
    status: "pending",
  };
}

export function useTrackLibrary() {
  const [tracks, setTracks] = useState<AudioTrackItem[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const browserFileInputRef = useRef<HTMLInputElement | null>(null);
  const browserFolderInputRef = useRef<HTMLInputElement | null>(null);

  const mergeTracks = useCallback(
    (incoming: AudioTrackItem[], skippedCount: number) => {
      let addedCount = 0;
      let duplicateCount = 0;

      setTracks((prev) => {
        const existingIds = new Set(prev.map((item) => item.id));
        const next = [...prev];

        for (const item of incoming) {
          if (existingIds.has(item.id)) {
            duplicateCount += 1;
          } else {
            existingIds.add(item.id);
            next.push(item);
            addedCount += 1;
          }
        }
        return next;
      });

      if (addedCount === 0 && incoming.length === 0) {
        setNotice(
          skippedCount > 0
            ? `Aucun fichier audio supporté trouvé (${skippedCount} ignoré${skippedCount > 1 ? "s" : ""})`
            : "Aucun fichier audio trouvé dans la sélection",
        );
        return;
      }

      const segments: string[] = [];
      if (addedCount > 0) {
        segments.push(
          `+${addedCount} piste${addedCount > 1 ? "s" : ""} ajoutée${addedCount > 1 ? "s" : ""}`,
        );
      }
      if (duplicateCount > 0) {
        segments.push(
          `${duplicateCount} déjà présente${duplicateCount > 1 ? "s" : ""}`,
        );
      }
      if (skippedCount > 0) {
        segments.push(
          `${skippedCount} non-audio ignoré${skippedCount > 1 ? "s" : ""}`,
        );
      }
      setNotice(segments.join(" · "));
    },
    [],
  );

  const importNativePaths = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      setErrorMessage(null);
      setIsScanning(true);
      try {
        const result = await scanAudioPaths(paths);
        mergeTracks(result.tracks, result.skippedCount);
      } catch (err) {
        setErrorMessage(
          err instanceof Error
            ? err.message
            : "Impossible de scanner les fichiers sélectionnés.",
        );
      } finally {
        setIsScanning(false);
      }
    },
    [mergeTracks],
  );

  const importBrowserFiles = useCallback(
    (files: FileList | File[]) => {
      setErrorMessage(null);
      const list = Array.from(files);
      if (list.length === 0) return;

      const parsed: AudioTrackItem[] = [];
      let skipped = 0;

      for (const file of list) {
        const track = buildBrowserTrack(file);
        if (track) {
          parsed.push(track);
        } else {
          skipped += 1;
        }
      }

      mergeTracks(parsed, skipped);
    },
    [mergeTracks],
  );

  const pickFolder = useCallback(async () => {
    setErrorMessage(null);
    if (!isTauriEnvironment()) {
      browserFolderInputRef.current?.click();
      return;
    }

    try {
      const selected = await openFolderDialog();
      if (selected.length > 0) {
        await importNativePaths(selected);
      }
    } catch (err) {
      setErrorMessage(
        err instanceof Error
          ? err.message
          : "Erreur lors de l'ouverture du sélecteur de dossier.",
      );
    }
  }, [importNativePaths]);

  const pickFiles = useCallback(async () => {
    setErrorMessage(null);
    if (!isTauriEnvironment()) {
      browserFileInputRef.current?.click();
      return;
    }

    try {
      const selected = await openAudioFilesDialog();
      if (selected.length > 0) {
        await importNativePaths(selected);
      }
    } catch (err) {
      setErrorMessage(
        err instanceof Error
          ? err.message
          : "Erreur lors de l'ouverture du sélecteur de fichiers.",
      );
    }
  }, [importNativePaths]);

  const removeTrack = useCallback((id: string) => {
    setTracks((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const clearTracks = useCallback(() => {
    setTracks([]);
    setNotice(null);
    setErrorMessage(null);
  }, []);

  // Native Tauri 2 Drag & Drop listener + Browser preview fallback
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    subscribeToWindowDragDrop({
      onEnter: () => {
        if (active) setIsDraggingOver(true);
      },
      onLeave: () => {
        if (active) setIsDraggingOver(false);
      },
      onDrop: (paths) => {
        if (active) {
          void importNativePaths(paths);
        }
      },
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    });

    // Always prevent the WebView from navigating away on raw file drop
    const handleDragOver = (event: DragEvent) => {
      event.preventDefault();
      if (!isTauriEnvironment()) {
        setIsDraggingOver(true);
      }
    };

    const handleDragLeave = (event: DragEvent) => {
      if (!isTauriEnvironment() && event.relatedTarget === null) {
        setIsDraggingOver(false);
      }
    };

    const handleDrop = (event: DragEvent) => {
      event.preventDefault();
      if (!isTauriEnvironment()) {
        setIsDraggingOver(false);
        if (event.dataTransfer?.files && event.dataTransfer.files.length > 0) {
          importBrowserFiles(event.dataTransfer.files);
        }
      }
    };

    window.addEventListener("dragover", handleDragOver);
    window.addEventListener("dragleave", handleDragLeave);
    window.addEventListener("drop", handleDrop);

    return () => {
      active = false;
      if (unlisten) unlisten();
      window.removeEventListener("dragover", handleDragOver);
      window.removeEventListener("dragleave", handleDragLeave);
      window.removeEventListener("drop", handleDrop);
    };
  }, [importNativePaths, importBrowserFiles]);

  const totalSizeBytes = useMemo(
    () => tracks.reduce((sum, item) => sum + item.sizeBytes, 0),
    [tracks],
  );

  const totalSizeLabel = useMemo(
    () => formatFileSize(totalSizeBytes),
    [totalSizeBytes],
  );

  const protectedCount = useMemo(
    () => tracks.filter((item) => item.hasProtectedKeyword).length,
    [tracks],
  );

  const reviewCount = useMemo(
    () => tracks.filter((item) => item.status === "needs_review").length,
    [tracks],
  );

  return {
    tracks,
    isScanning,
    isDraggingOver,
    notice,
    errorMessage,
    clearError: () => setErrorMessage(null),
    totalSizeLabel,
    protectedCount,
    reviewCount,
    pickFolder,
    pickFiles,
    removeTrack,
    clearTracks,
    importBrowserFiles,
    browserFileInputRef,
    browserFolderInputRef,
  };
}

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  isTauriEnvironment,
  lookupTrackMetadata,
  openAudioFilesDialog,
  openExportDirectory,
  openFolderDialog,
  runBatchLookup,
  runBatchPipeline,
  scanAudioPaths,
  subscribeToPipelineProgress,
  subscribeToWindowDragDrop,
} from "../services/tauriBridge";
import type {
  AudioTrackItem,
  BatchSummary,
  BatchTrackInput,
  ExportMode,
  ManualExportResult,
  PipelineProgressEvent,
  QueueStats,
  TrackMetadata,
} from "../types/track";
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
  const [batchAction, setBatchAction] = useState<"lookup" | "export" | null>(
    null,
  );
  const isRunningBatch = batchAction !== null;
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const [batchProgress, setBatchProgress] =
    useState<PipelineProgressEvent | null>(null);
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

      if (addedCount > 0) {
        setBatchProgress(null);
      }

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
    setBatchProgress(null);
    setNotice(null);
    setErrorMessage(null);
  }, []);

  const lookupTrack = useCallback(
    async (id: string) => {
      const target = tracks.find((t) => t.id === id);
      if (!target) return;
      if (!isTauriEnvironment()) {
        setErrorMessage(
          "La recherche Apple Music (US / UK / FR) nécessite l'application desktop Tauri.",
        );
        return;
      }

      setErrorMessage(null);
      setTracks((prev) =>
        prev.map((item) =>
          item.id === id
            ? { ...item, status: "processing", reviewReason: undefined }
            : item,
        ),
      );

      try {
        const result = await lookupTrackMetadata(target.sourcePath);
        if (result.status === "matched" && result.metadata) {
          setTracks((prev) =>
            prev.map((item) =>
              item.id === id
                ? {
                    ...item,
                    status: "completed",
                    metadata: result.metadata,
                    reviewReason: undefined,
                    candidates: undefined,
                    lastQueryUsed: result.queryUsed,
                  }
                : item,
            ),
          );
          setNotice(
            `Apple Music ${result.storefront ?? "US"} : « ${result.metadata.artist} — ${result.metadata.title} » (aperçu lecture seule)`,
          );
        } else {
          setTracks((prev) =>
            prev.map((item) =>
              item.id === id
                ? {
                    ...item,
                    status: "needs_review",
                    reviewReason: result.reason,
                    candidates: result.candidates ?? [],
                    lastQueryUsed: result.queryUsed,
                  }
                : item,
            ),
          );
          if (result.reason) {
            setNotice(result.reason);
          }
        }
      } catch (err) {
        const message =
          err instanceof Error
            ? err.message
            : "Erreur lors de l'appel Apple Music (US / UK / FR).";
        setTracks((prev) =>
          prev.map((item) =>
            item.id === id
              ? { ...item, status: "failed", reviewReason: message }
              : item,
          ),
        );
        setErrorMessage(message);
      }
    },
    [tracks],
  );

  const startBatchLookup = useCallback(async () => {
    if (isRunningBatch || tracks.length === 0) return;

    if (!isTauriEnvironment()) {
      setErrorMessage(
        "L'identification Apple Music nécessite l'application desktop Tauri.",
      );
      return;
    }

    const targetTracks = tracks.filter(
      (t) => t.status === "pending" || t.status === "failed",
    );
    if (targetTracks.length === 0) {
      setNotice("Toutes les pistes en attente ont déjà été analysées.");
      return;
    }

    const inputs: BatchTrackInput[] = targetTracks.map((t) => ({
      id: t.id,
      sourcePath: t.sourcePath,
    }));

    setErrorMessage(null);
    setBatchAction("lookup");
    setBatchProgress(null);

    try {
      const summary = await runBatchLookup(inputs);
      const parts: string[] = [];
      if (summary.completedCount > 0) {
        parts.push(
          `${summary.completedCount} identifiée${summary.completedCount > 1 ? "s" : ""}`,
        );
      }
      if (summary.needsReviewCount > 0) {
        parts.push(`${summary.needsReviewCount} à réviser`);
      }
      if (summary.failedCount > 0) {
        parts.push(
          `${summary.failedCount} erreur${summary.failedCount > 1 ? "s" : ""}`,
        );
      }
      setNotice(
        parts.length > 0
          ? `Identification terminée : ${parts.join(" · ")}`
          : "Identification terminée",
      );
    } catch (err) {
      const message =
        err instanceof Error
          ? err.message
          : "Erreur pendant l'identification par lots.";
      setErrorMessage(message);
    } finally {
      setBatchAction(null);
    }
  }, [isRunningBatch, tracks]);

  const startBatchPipeline = useCallback(
    async (
      exportMode: ExportMode,
      options?: { forceAll?: boolean },
    ): Promise<BatchSummary | null> => {
      if (isRunningBatch || tracks.length === 0) return null;

      if (!isTauriEnvironment()) {
        setErrorMessage(
          "L'export non destructif nécessite l'application desktop Tauri.",
        );
        return null;
      }

      const forceAll = options?.forceAll ?? false;
      const identifiedTracks = tracks.filter(
        (t) => t.status === "completed" && !t.exportedPath,
      );
      const pendingOrFailed = tracks.filter(
        (t) => !t.exportedPath && t.status !== "needs_review",
      );
      const targetTracks = forceAll
        ? tracks
        : identifiedTracks.length > 0
          ? identifiedTracks
          : pendingOrFailed;

      if (targetTracks.length === 0) {
        setNotice(
          "Aucune piste prête à exporter — utilisez « Tout relancer » pour ré-exporter.",
        );
        return null;
      }

      if (forceAll) {
        const targetIds = new Set(targetTracks.map((t) => t.id));
        setTracks((prev) =>
          prev.map((item) =>
            targetIds.has(item.id)
              ? {
                  ...item,
                  status: "pending",
                  exportedPath: undefined,
                  reviewReason: undefined,
                }
              : item,
          ),
        );
      }

      const inputs: BatchTrackInput[] = targetTracks.map((t) => ({
        id: t.id,
        sourcePath: t.sourcePath,
        cachedMetadata:
          t.status === "needs_review" || t.status === "failed"
            ? undefined
            : t.metadata,
        localArtworkPath: t.localArtworkPath,
      }));

      setErrorMessage(null);
      setBatchAction("export");
      setBatchProgress(null);

      try {
        const summary = await runBatchPipeline(inputs, exportMode);

        const parts = [
          `${summary.completedCount} exportée${summary.completedCount > 1 ? "s" : ""}`,
        ];
        if (summary.needsReviewCount > 0) {
          parts.push(`${summary.needsReviewCount} à réviser`);
        }
        if (summary.failedCount > 0) {
          parts.push(
            `${summary.failedCount} erreur${summary.failedCount > 1 ? "s" : ""}`,
          );
        }
        setNotice(`Lot terminé : ${parts.join(" · ")}`);
        return summary;
      } catch (err) {
        const message =
          err instanceof Error
            ? err.message
            : "Erreur pendant le traitement par lots.";
        setErrorMessage(message);
        return null;
      } finally {
        setBatchAction(null);
      }
    },
    [isRunningBatch, tracks],
  );

  const openExportFolder = useCallback(async () => {
    setErrorMessage(null);
    if (!isTauriEnvironment()) {
      setErrorMessage(
        "L'ouverture du dossier local nécessite l'application desktop Tauri.",
      );
      return;
    }

    try {
      const openedPath = await openExportDirectory();
      setNotice(`Dossier d'export ouvert : ${openedPath}`);
    } catch (err) {
      setErrorMessage(
        err instanceof Error
          ? err.message
          : "Impossible d'ouvrir le dossier Tagmaster_DJ_Export.",
      );
    }
  }, []);

  const validateTrackFromReview = useCallback(
    (trackId: string, metadata: TrackMetadata, localArtworkPath?: string) => {
      setErrorMessage(null);
      setTracks((prev) =>
        prev.map((item) =>
          item.id === trackId
            ? {
                ...item,
                status: "completed",
                metadata,
                localArtworkPath: localArtworkPath || undefined,
                exportedPath: undefined,
                reviewReason: undefined,
                candidates: undefined,
              }
            : item,
        ),
      );
      setNotice(
        `Tags validés (prêt pour export) : « ${metadata.artist} — ${metadata.title} »`,
      );
    },
    [],
  );

  const resolveTrackFromReview = useCallback(
    (trackId: string, result: ManualExportResult) => {
      setErrorMessage(null);
      setTracks((prev) =>
        prev.map((item) =>
          item.id === trackId
            ? {
                ...item,
                status: "completed",
                metadata: result.metadata,
                exportedPath: result.exportedPath,
                localArtworkPath: undefined,
                reviewReason: undefined,
                candidates: undefined,
              }
            : item,
        ),
      );
      setNotice(
        `Exporté depuis la File de révision : « ${result.metadata.artist} — ${result.metadata.title} »`,
      );
    },
    [],
  );

  // Listen to real-time pipeline progress events from Rust
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    subscribeToPipelineProgress((event) => {
      if (!active) return;
      setBatchProgress(event);
      setTracks((prev) =>
        prev.map((item) => {
          if (item.id !== event.trackId) return item;
          const clearReview =
            event.trackStatus === "processing" ||
            event.trackStatus === "completed";
          return {
            ...item,
            status: event.trackStatus,
            metadata: event.metadata ?? item.metadata,
            exportedPath: event.exportedPath ?? item.exportedPath,
            reviewReason: clearReview ? undefined : event.reviewReason,
            candidates: clearReview
              ? undefined
              : event.candidates && event.candidates.length > 0
                ? event.candidates
                : item.candidates,
            lastQueryUsed: event.queryUsed ?? item.lastQueryUsed,
          };
        }),
      );
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    });

    return () => {
      active = false;
      if (unlisten) unlisten();
    };
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

  const queueStats = useMemo<QueueStats>(() => {
    let pendingCount = 0;
    let identifiedCount = 0;
    let exportedCount = 0;
    let needsReviewCount = 0;
    let failedCount = 0;
    let processingCount = 0;
    let identifiableCount = 0;

    for (const item of tracks) {
      if (item.status === "completed" && item.exportedPath) {
        exportedCount += 1;
      } else if (item.status === "completed") {
        identifiedCount += 1;
      } else if (item.status === "needs_review") {
        needsReviewCount += 1;
      } else if (item.status === "failed") {
        failedCount += 1;
      } else if (item.status === "processing") {
        processingCount += 1;
      } else {
        pendingCount += 1;
      }

      if (item.status === "pending" || item.status === "failed") {
        identifiableCount += 1;
      }
    }

    const exportableCount =
      identifiedCount > 0 ? identifiedCount : identifiableCount;

    return {
      totalCount: tracks.length,
      pendingCount,
      identifiedCount,
      exportedCount,
      needsReviewCount,
      failedCount,
      processingCount,
      identifiableCount,
      exportableCount,
    };
  }, [tracks]);

  const reviewCount = queueStats.needsReviewCount + queueStats.failedCount;

  return {
    tracks,
    queueStats,
    isScanning,
    isRunningBatch,
    batchAction,
    isDraggingOver,
    batchProgress,
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
    lookupTrack,
    startBatchLookup,
    startBatchPipeline,
    openExportFolder,
    validateTrackFromReview,
    resolveTrackFromReview,
    importBrowserFiles,
    browserFileInputRef,
    browserFolderInputRef,
  };
}

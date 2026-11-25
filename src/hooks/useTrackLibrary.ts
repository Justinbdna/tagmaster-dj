import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  clearSessionCache,
  detectDuplicateGroups,
  exitApp,
  getSessionCacheSummary,
  isTauriEnvironment,
  loadSessionCache,
  lookupTrackMetadata,
  openAudioFilesDialog,
  openExportDirectory,
  openFolderDialog,
  runBatchLookup,
  runBatchPipeline,
  saveSessionCache,
  scanAudioPaths,
  subscribeToPipelineProgress,
  subscribeToWindowDragDrop,
} from "../services/tauriBridge";
import type {
  AudioTrackItem,
  BatchSummary,
  BatchTrackInput,
  DuplicateGroup,
  ExportFormatMode,
  ExportMode,
  HardwareProfile,
  ManualExportResult,
  NetworkInterruptionState,
  PipelineProgressEvent,
  QueueStats,
  SessionCache,
  SessionCacheSummary,
  TrackMetadata,
} from "../types/track";
import { formatFileSize } from "../utils/format";

const PROTECTED_KEYWORDS = [
  "live",
  "acoustic",
  "sped up",
  "slowed",
  "reverb",
  "nightcore",
  "chopped",
  "instrumental",
  "remix",
  "mashup",
];

const SUPPORTED_EXT_SET = new Set(["MP3", "M4A", "AAC", "FLAC", "WAV"]);
const BROWSER_CACHE_KEY = "tagmaster_dj_session_cache_v1";

export function makeDuplicatePairKey(idA: string, idB: string): string {
  return idA < idB ? `${idA}|||${idB}` : `${idB}|||${idA}`;
}

function normalizeForDupFallback(raw: string): string {
  return raw
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/['’‘`]/g, "")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function cleanStemFallback(fileName: string): string {
  const stem = fileName.replace(/\.[^.]+$/, "");
  return stem
    .replace(/^\s*\d{1,3}\s*[-._–—)\]]\s*/, "")
    .replace(/\s*(?:[-–—]\s*)?(?:copy|copie)(?:\s*\d{1,2})?\s*$/i, "")
    .replace(/\s*\(\d{1,2}\)\s*$/, "")
    .replace(/__+\d{1,2}\s*$/, "")
    .replace(/_(\d{1,2})\s*$/, "")
    .replace(/_/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function detectDuplicatesBrowserFallback(
  tracks: AudioTrackItem[],
): DuplicateGroup[] {
  const buckets = new Map<
    string,
    { label: string; reason: string; items: AudioTrackItem[] }
  >();

  for (const track of tracks) {
    let key: string;
    let label: string;
    let reason: string;

    if (track.metadata) {
      const normArtist = normalizeForDupFallback(track.metadata.artist);
      const cleanTitle = track.metadata.title
        .replace(/\s*[([]\s*(?:feat\.?|ft\.?|featuring)\s+[^)\]]+[)\]]?/gi, "")
        .trim();
      const normTitle = normalizeForDupFallback(cleanTitle);
      key = `at::${normArtist}::${normTitle}`;
      label = `${track.metadata.artist} — ${cleanTitle}`;
      reason = "Même artiste & titre identifiés";
    } else {
      const cleaned = cleanStemFallback(track.fileName);
      const norm = normalizeForDupFallback(cleaned);
      if (!norm) continue;
      key = `stem::${norm}`;
      label = cleaned;
      reason = "Nom normalisé identique (variante / copie)";
    }

    const existing = buckets.get(key);
    if (existing) {
      existing.items.push(track);
    } else {
      buckets.set(key, { label, reason, items: [track] });
    }
  }

  const groups: DuplicateGroup[] = [];
  for (const [groupKey, bucket] of buckets.entries()) {
    if (bucket.items.length < 2) continue;
    groups.push({
      groupKey,
      displayLabel: bucket.label,
      matchReason: bucket.reason,
      trackIds: bucket.items.map((t) => t.id),
    });
  }
  return groups;
}

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

function summarizeBrowserSession(session: SessionCache): SessionCacheSummary {
  let exportedCount = 0;
  let identifiedCount = 0;
  let needsReviewCount = 0;

  for (const track of session.tracks) {
    if (track.status === "completed" && track.exportedPath) {
      exportedCount += 1;
    } else if (track.status === "completed") {
      identifiedCount += 1;
    } else if (track.status === "needs_review" || track.status === "failed") {
      needsReviewCount += 1;
    }
  }

  const totalTracks = session.tracks.length;
  const remainingCount = Math.max(0, totalTracks - exportedCount);

  return {
    hasSession: totalTracks > 0,
    isUnfinished: totalTracks > 0 && remainingCount > 0,
    updatedAtMs: session.updatedAtMs,
    exportMode: session.exportMode,
    totalTracks,
    exportedCount,
    identifiedCount,
    needsReviewCount,
    remainingCount,
    cacheFilePath: "localStorage (preview)",
  };
}

export function useTrackLibrary(
  exportMode: ExportMode,
  onRestoreExportMode: (mode: ExportMode) => void,
  formatMode: ExportFormatMode = "passthrough",
  hardwareProfile: HardwareProfile = "standard",
) {
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

  // Sprint 2 Bloc 3 — Session persistence (#13)
  const [pendingSessionSummary, setPendingSessionSummary] =
    useState<SessionCacheSummary | null>(null);
  const [isResumedSession, setIsResumedSession] = useState(false);
  const sessionReadyRef = useRef(false);
  const tracksRef = useRef<AudioTrackItem[]>(tracks);
  tracksRef.current = tracks;
  const exportModeRef = useRef<ExportMode>(exportMode);
  exportModeRef.current = exportMode;
  const formatModeRef = useRef<ExportFormatMode>(formatMode);
  formatModeRef.current = formatMode;
  const hardwareProfileRef = useRef<HardwareProfile>(hardwareProfile);
  hardwareProfileRef.current = hardwareProfile;

  // Sprint 2 Bloc 3 — Network resilience modal (#13)
  const [networkInterruption, setNetworkInterruption] =
    useState<NetworkInterruptionState | null>(null);

  // Sprint 2 Bloc 3 — Duplicate detection (#14)
  const [rawDuplicateGroups, setRawDuplicateGroups] = useState<
    DuplicateGroup[]
  >([]);
  const [dismissedDuplicatePairs, setDismissedDuplicatePairs] = useState<
    Set<string>
  >(() => new Set());

  const browserFileInputRef = useRef<HTMLInputElement | null>(null);
  const browserFolderInputRef = useRef<HTMLInputElement | null>(null);

  const persistSessionNow = useCallback(
    async (tracksToSave: AudioTrackItem[], modeToSave: ExportMode) => {
      const payload: SessionCache = {
        version: 1,
        updatedAtMs: Date.now(),
        exportMode: modeToSave,
        tracks: tracksToSave,
      };

      if (isTauriEnvironment()) {
        try {
          if (tracksToSave.length === 0) {
            await clearSessionCache();
          } else {
            await saveSessionCache(payload);
          }
        } catch {
          // Non-blocking persistence failure
        }
      } else {
        try {
          if (tracksToSave.length === 0) {
            window.localStorage.removeItem(BROWSER_CACHE_KEY);
          } else {
            window.localStorage.setItem(
              BROWSER_CACHE_KEY,
              JSON.stringify(payload),
            );
          }
        } catch {
          // Ignore storage quota errors in browser preview
        }
      }
    },
    [],
  );

  // Check for an unfinished saved session on startup
  useEffect(() => {
    let active = true;

    async function checkSavedSession() {
      if (isTauriEnvironment()) {
        try {
          const summary = await getSessionCacheSummary();
          if (!active) return;
          if (summary.hasSession && summary.isUnfinished) {
            setPendingSessionSummary(summary);
          } else {
            sessionReadyRef.current = true;
          }
        } catch {
          if (active) sessionReadyRef.current = true;
        }
      } else {
        try {
          const raw = window.localStorage.getItem(BROWSER_CACHE_KEY);
          if (raw) {
            const parsed = JSON.parse(raw) as SessionCache;
            const summary = summarizeBrowserSession(parsed);
            if (active && summary.hasSession && summary.isUnfinished) {
              setPendingSessionSummary(summary);
              return;
            }
          }
        } catch {
          // Ignore invalid preview storage
        }
        if (active) sessionReadyRef.current = true;
      }
    }

    void checkSavedSession();
    return () => {
      active = false;
    };
  }, []);

  // Auto-save cache.json whenever tracks or exportMode change (once hydrated)
  useEffect(() => {
    if (!sessionReadyRef.current) return;
    const timer = window.setTimeout(() => {
      void persistSessionNow(tracks, exportMode);
    }, 180);
    return () => window.clearTimeout(timer);
  }, [tracks, exportMode, persistSessionNow]);

  const resumePreviousSession = useCallback(async (): Promise<boolean> => {
    try {
      let restored: SessionCache | null = null;
      if (isTauriEnvironment()) {
        restored = await loadSessionCache();
      } else {
        const raw = window.localStorage.getItem(BROWSER_CACHE_KEY);
        if (raw) restored = JSON.parse(raw) as SessionCache;
      }

      if (restored && restored.tracks.length > 0) {
        onRestoreExportMode(restored.exportMode);
        setTracks(restored.tracks);
        setIsResumedSession(true);
        const exported = restored.tracks.filter(
          (t) => t.status === "completed" && Boolean(t.exportedPath),
        ).length;
        const remaining = restored.tracks.length - exported;
        setNotice(
          `Session reprise — ${exported} piste${exported > 1 ? "s" : ""} déjà traitée${exported > 1 ? "s" : ""} · ${remaining} restante${remaining > 1 ? "s" : ""}`,
        );
      }
      setPendingSessionSummary(null);
      sessionReadyRef.current = true;
      return Boolean(restored && restored.tracks.length > 0);
    } catch (err) {
      setErrorMessage(
        err instanceof Error
          ? err.message
          : "Impossible de charger la session précédente.",
      );
      setPendingSessionSummary(null);
      sessionReadyRef.current = true;
      return false;
    }
  }, [onRestoreExportMode]);

  const startNewSession = useCallback(async () => {
    setPendingSessionSummary(null);
    setIsResumedSession(false);
    setTracks([]);
    setDismissedDuplicatePairs(new Set());
    sessionReadyRef.current = true;
    await persistSessionNow([], exportModeRef.current);
  }, [persistSessionNow]);

  // Duplicate detection whenever tracks change
  const duplicateSignatureKey = useMemo(
    () =>
      tracks
        .map(
          (t) =>
            `${t.id}|${t.fileName}|${t.extension}|${t.sizeBytes}|${t.bitrateKbps ?? ""}|${t.metadata?.artist ?? ""}|${t.metadata?.title ?? ""}`,
        )
        .join("\n"),
    [tracks],
  );

  useEffect(() => {
    let active = true;
    if (tracks.length < 2) {
      setRawDuplicateGroups([]);
      return;
    }

    if (isTauriEnvironment()) {
      detectDuplicateGroups(tracks)
        .then((groups) => {
          if (active) setRawDuplicateGroups(groups);
        })
        .catch(() => {
          if (active) {
            setRawDuplicateGroups(detectDuplicatesBrowserFallback(tracks));
          }
        });
    } else {
      setRawDuplicateGroups(detectDuplicatesBrowserFallback(tracks));
    }

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [duplicateSignatureKey]);

  // Filter out groups where every pair has been explicitly dismissed ("Garder les deux")
  const duplicateGroups = useMemo<DuplicateGroup[]>(() => {
    const existingIds = new Set(tracks.map((t) => t.id));
    return rawDuplicateGroups
      .map((group) => ({
        ...group,
        trackIds: group.trackIds.filter((id) => existingIds.has(id)),
      }))
      .filter((group) => {
        if (group.trackIds.length < 2) return false;
        for (let i = 0; i < group.trackIds.length; i += 1) {
          for (let j = i + 1; j < group.trackIds.length; j += 1) {
            const pairKey = makeDuplicatePairKey(
              group.trackIds[i],
              group.trackIds[j],
            );
            if (!dismissedDuplicatePairs.has(pairKey)) {
              return true;
            }
          }
        }
        return false;
      });
  }, [rawDuplicateGroups, tracks, dismissedDuplicatePairs]);

  const duplicateGroupByTrackId = useMemo(() => {
    const map = new Map<string, DuplicateGroup>();
    for (const group of duplicateGroups) {
      for (const id of group.trackIds) {
        map.set(id, group);
      }
    }
    return map;
  }, [duplicateGroups]);

  const dismissDuplicatePair = useCallback((idA: string, idB: string) => {
    const key = makeDuplicatePairKey(idA, idB);
    setDismissedDuplicatePairs((prev) => {
      const next = new Set(prev);
      next.add(key);
      return next;
    });
  }, []);

  const mergeTracks = useCallback(
    (incoming: AudioTrackItem[], skippedCount: number) => {
      let addedCount = 0;
      let duplicateCount = 0;

      // User dropped files directly while resume modal wasn't dismissed -> activate session
      if (!sessionReadyRef.current) {
        setPendingSessionSummary(null);
        sessionReadyRef.current = true;
      }

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

  const removeTracks = useCallback((ids: string[]) => {
    if (ids.length === 0) return;
    const dropSet = new Set(ids);
    setTracks((prev) => prev.filter((item) => !dropSet.has(item.id)));
  }, []);

  const clearTracks = useCallback(() => {
    setTracks([]);
    setBatchProgress(null);
    setNotice(null);
    setErrorMessage(null);
    setIsResumedSession(false);
    setNetworkInterruption(null);
    setDismissedDuplicatePairs(new Set());
    void persistSessionNow([], exportModeRef.current);
  }, [persistSessionNow]);

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

  const buildInterruptionState = useCallback(
    (
      action: "lookup" | "export",
      summary: BatchSummary,
    ): NetworkInterruptionState => {
      const current = tracksRef.current;
      const exportedCount = current.filter(
        (t) => t.status === "completed" && Boolean(t.exportedPath),
      ).length;
      const readyToExportCount = current.filter(
        (t) => t.status === "completed" && !t.exportedPath,
      ).length;
      const pendingOrFailedCount = current.filter(
        (t) => t.status === "pending" || t.status === "failed",
      ).length;

      return {
        action,
        errorMessage:
          summary.networkErrorMessage ??
          "Coupure réseau ou limite de requêtes API détectée.",
        remainingCount: Math.max(
          summary.remainingCount ?? 0,
          pendingOrFailedCount,
        ),
        readyToExportCount,
        exportedCount,
      };
    },
    [],
  );

  const startBatchLookup = useCallback(async (): Promise<BatchSummary | null> => {
    if (isRunningBatch || tracksRef.current.length === 0) return null;

    if (!isTauriEnvironment()) {
      setErrorMessage(
        "L'identification Apple Music nécessite l'application desktop Tauri.",
      );
      return null;
    }

    const targetTracks = tracksRef.current.filter(
      (t) => t.status === "pending" || t.status === "failed",
    );
    if (targetTracks.length === 0) {
      setNotice("Toutes les pistes en attente ont déjà été analysées.");
      return null;
    }

    const inputs: BatchTrackInput[] = targetTracks.map((t) => ({
      id: t.id,
      sourcePath: t.sourcePath,
    }));

    setErrorMessage(null);
    setNetworkInterruption(null);
    setBatchAction("lookup");
    setBatchProgress(null);

    try {
      const summary = await runBatchLookup(inputs);
      await persistSessionNow(tracksRef.current, exportModeRef.current);

      if (summary.interruptedByNetwork) {
        setNetworkInterruption(buildInterruptionState("lookup", summary));
        setNotice("Identification mise en pause — progression sauvegardée");
        return summary;
      }

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
      return summary;
    } catch (err) {
      const message =
        err instanceof Error
          ? err.message
          : "Erreur pendant l'identification par lots.";
      setErrorMessage(message);
      return null;
    } finally {
      setBatchAction(null);
    }
  }, [buildInterruptionState, isRunningBatch, persistSessionNow]);

  const startBatchPipeline = useCallback(
    async (
      mode: ExportMode,
      options?: { forceAll?: boolean; onlyIdentified?: boolean },
    ): Promise<BatchSummary | null> => {
      const currentTracks = tracksRef.current;
      if (isRunningBatch || currentTracks.length === 0) return null;

      if (!isTauriEnvironment()) {
        setErrorMessage(
          "L'export non destructif nécessite l'application desktop Tauri.",
        );
        return null;
      }

      const forceAll = options?.forceAll ?? false;
      const onlyIdentified = options?.onlyIdentified ?? false;
      const identifiedTracks = currentTracks.filter(
        (t) => t.status === "completed" && !t.exportedPath,
      );
      const pendingOrFailed = currentTracks.filter(
        (t) => !t.exportedPath && t.status !== "needs_review",
      );
      const targetTracks = onlyIdentified
        ? identifiedTracks
        : forceAll
          ? currentTracks
          : identifiedTracks.length > 0
            ? identifiedTracks
            : pendingOrFailed;

      if (targetTracks.length === 0) {
        setNotice(
          onlyIdentified
            ? "Aucune piste identifiée prête à exporter."
            : "Aucune piste prête à exporter — utilisez « Tout relancer » pour ré-exporter.",
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
      setNetworkInterruption(null);
      setBatchAction("export");
      setBatchProgress(null);

      try {
        const summary = await runBatchPipeline(
          inputs,
          mode,
          formatModeRef.current,
          hardwareProfileRef.current,
        );
        await persistSessionNow(tracksRef.current, mode);

        if (summary.interruptedByNetwork) {
          setNetworkInterruption(buildInterruptionState("export", summary));
          setNotice("Export mis en pause — progression sauvegardée");
          return summary;
        }

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
    [buildInterruptionState, isRunningBatch, persistSessionNow],
  );

  const retryInterruptedBatch = useCallback(async (): Promise<BatchSummary | null> => {
    const action = networkInterruption?.action ?? "lookup";
    setNetworkInterruption(null);
    if (action === "export") {
      return startBatchPipeline(exportModeRef.current);
    }
    return startBatchLookup();
  }, [networkInterruption, startBatchLookup, startBatchPipeline]);

  const exportReadyTracksFromInterruption =
    useCallback(async (): Promise<BatchSummary | null> => {
      setNetworkInterruption(null);
      return startBatchPipeline(exportModeRef.current, {
        onlyIdentified: true,
      });
    }, [startBatchPipeline]);

  const quitAfterSaving = useCallback(async () => {
    await persistSessionNow(tracksRef.current, exportModeRef.current);
    setNetworkInterruption(null);
    if (isTauriEnvironment()) {
      await exitApp();
    }
  }, [persistSessionNow]);

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
    (trackId: string, result: ManualExportResult, localArtworkPath?: string) => {
      setErrorMessage(null);
      setTracks((prev) =>
        prev.map((item) =>
          item.id === trackId
            ? {
                ...item,
                status: "completed",
                metadata: result.metadata,
                exportedPath: result.exportedPath,
                localArtworkPath: result.metadata.artworkUrl
                  ? undefined
                  : localArtworkPath || item.localArtworkPath,
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

  // Listen to real-time pipeline progress events from Rust + save cache.json after each completed track
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let active = true;

    subscribeToPipelineProgress((event) => {
      if (!active) return;
      setBatchProgress(event);
      setTracks((prev) => {
        const next = prev.map((item) => {
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
        });
        tracksRef.current = next;
        if (event.trackStatus !== "processing" && sessionReadyRef.current) {
          void persistSessionNow(next, exportModeRef.current);
        }
        return next;
      });
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
  }, [persistSessionNow]);

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
    pendingSessionSummary,
    isResumedSession,
    resumePreviousSession,
    startNewSession,
    networkInterruption,
    dismissNetworkInterruption: () => setNetworkInterruption(null),
    retryInterruptedBatch,
    exportReadyTracksFromInterruption,
    quitAfterSaving,
    duplicateGroups,
    duplicateGroupByTrackId,
    dismissedDuplicatePairs,
    dismissDuplicatePair,
    pickFolder,
    pickFiles,
    removeTrack,
    removeTracks,
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

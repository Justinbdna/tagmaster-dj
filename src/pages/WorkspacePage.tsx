import { useCallback, useMemo, useState } from "react";
import { AlertCircle, X } from "lucide-react";
import { DragOverlay } from "../components/dropzone/DragOverlay";
import { DropZoneCard } from "../components/dropzone/DropZoneCard";
import { DuplicateComparatorView } from "../components/duplicates/DuplicateComparatorView";
import { GlassShell } from "../components/layout/GlassShell";
import { Sidebar } from "../components/layout/Sidebar";
import { TopBar } from "../components/layout/TopBar";
import { NetworkResilienceModal } from "../components/modals/NetworkResilienceModal";
import { SessionResumeModal } from "../components/modals/SessionResumeModal";
import { FloatingActionDock } from "../components/progress/FloatingActionDock";
import { ReviewQueueView } from "../components/review/ReviewQueueView";
import { SettingsView } from "../components/settings/SettingsView";
import { TrackQueueList } from "../components/tracks/TrackQueueList";
import {
  ToastNotification,
  type ToastPayload,
} from "../components/ui/ToastNotification";
import { useAppHealth } from "../hooks/useAppHealth";
import { useTheme } from "../hooks/useTheme";
import { useTrackLibrary } from "../hooks/useTrackLibrary";
import type {
  AppView,
  ExportFormatMode,
  ExportMode,
  HardwareProfile,
  ManualExportResult,
  TrackMetadata,
} from "../types/track";

const FORMAT_STORAGE_KEY = "tagmaster_dj_format_mode";
const PROFILE_STORAGE_KEY = "tagmaster_dj_hardware_profile";

export function WorkspacePage() {
  const [activeView, setActiveView] = useState<AppView>("library");
  const [exportMode, setExportMode] = useState<ExportMode>("dynamic");
  const [formatMode, setFormatModeState] = useState<ExportFormatMode>(() => {
    const saved = window.localStorage.getItem(FORMAT_STORAGE_KEY);
    return saved === "force_mp3" ? "force_mp3" : "passthrough";
  });
  const [hardwareProfile, setHardwareProfileState] = useState<HardwareProfile>(
    () => {
      const saved = window.localStorage.getItem(PROFILE_STORAGE_KEY);
      if (saved === "ipod" || saved === "psp") return saved;
      return "standard";
    },
  );
  const [selectedReviewTrackId, setSelectedReviewTrackId] = useState<
    string | null
  >(null);
  const [selectedDuplicateGroupKey, setSelectedDuplicateGroupKey] = useState<
    string | null
  >(null);
  const [toast, setToast] = useState<ToastPayload | null>(null);

  const setFormatMode = useCallback((mode: ExportFormatMode) => {
    setFormatModeState(mode);
    window.localStorage.setItem(FORMAT_STORAGE_KEY, mode);
  }, []);

  const setHardwareProfile = useCallback((profile: HardwareProfile) => {
    setHardwareProfileState(profile);
    window.localStorage.setItem(PROFILE_STORAGE_KEY, profile);
  }, []);

  const { health } = useAppHealth();
  const { theme, setTheme, toggleTheme, glassTint, setGlassTint } = useTheme();
  const library = useTrackLibrary(
    exportMode,
    setExportMode,
    formatMode,
    hardwareProfile,
  );

  const isFocusMode = activeView === "review" || activeView === "duplicates";

  const showToast = useCallback((message: string) => {
    setToast({ id: Date.now(), message });
  }, []);

  const openTrackInReview = useCallback((trackId: string) => {
    setSelectedReviewTrackId(trackId);
    setActiveView("review");
  }, []);

  const openDuplicateComparator = useCallback((groupKey?: string) => {
    setSelectedDuplicateGroupKey(groupKey ?? null);
    setActiveView("duplicates");
  }, []);

  const reviewTracks = useMemo(
    () =>
      library.tracks.filter(
        (t) => t.status === "needs_review" || t.status === "failed",
      ),
    [library.tracks],
  );

  const editorTracks = useMemo(() => {
    if (!selectedReviewTrackId) return reviewTracks;
    const isInReview = reviewTracks.some((t) => t.id === selectedReviewTrackId);
    if (isInReview) return reviewTracks;
    const selectedFreeEdit = library.tracks.find(
      (t) => t.id === selectedReviewTrackId,
    );
    return selectedFreeEdit
      ? [selectedFreeEdit, ...reviewTracks]
      : reviewTracks;
  }, [library.tracks, reviewTracks, selectedReviewTrackId]);

  const handleRunBatchExport = useCallback(
    async (options?: { forceAll?: boolean }) => {
      const summary = await library.startBatchPipeline(exportMode, options);
      if (summary && !summary.interruptedByNetwork && summary.completedCount > 0) {
        showToast(
          `Export terminé — ${summary.completedCount} piste${summary.completedCount > 1 ? "s" : ""} exportée${summary.completedCount > 1 ? "s" : ""}`,
        );
      }
    },
    [exportMode, library.startBatchPipeline, showToast],
  );

  const handleResumeSession = useCallback(async () => {
    const resumed = await library.resumePreviousSession();
    if (resumed) {
      showToast("Session précédente restaurée");
    }
  }, [library.resumePreviousSession, showToast]);

  const handleStartNewSession = useCallback(async () => {
    await library.startNewSession();
    showToast("Nouvelle session démarrée");
  }, [library.startNewSession, showToast]);

  const handleRetryInterruptedBatch = useCallback(async () => {
    const summary = await library.retryInterruptedBatch();
    if (
      summary &&
      !summary.interruptedByNetwork &&
      summary.exportDirectory &&
      summary.completedCount > 0
    ) {
      showToast(
        `Export terminé — ${summary.completedCount} piste${summary.completedCount > 1 ? "s" : ""} exportée${summary.completedCount > 1 ? "s" : ""}`,
      );
    }
  }, [library.retryInterruptedBatch, showToast]);

  const handleExportReadyFromInterruption = useCallback(async () => {
    const summary = await library.exportReadyTracksFromInterruption();
    if (summary && summary.completedCount > 0) {
      showToast(
        `Export partiel terminé — ${summary.completedCount} piste${summary.completedCount > 1 ? "s" : ""} exportée${summary.completedCount > 1 ? "s" : ""}`,
      );
    }
  }, [library.exportReadyTracksFromInterruption, showToast]);

  const handleTrackValidatedFromReview = useCallback(
    (trackId: string, metadata: TrackMetadata, localArtworkPath?: string) => {
      const wasInReviewQueue = reviewTracks.some((t) => t.id === trackId);
      const remainingList = reviewTracks.filter((t) => t.id !== trackId);
      library.validateTrackFromReview(trackId, metadata, localArtworkPath);

      if (!wasInReviewQueue) {
        setSelectedReviewTrackId(null);
        setActiveView("library");
        showToast("Tags mis à jour — piste prête à exporter");
      } else if (remainingList.length > 0) {
        setSelectedReviewTrackId(remainingList[0].id);
        showToast("Tags validés — piste prête à exporter");
      } else {
        setSelectedReviewTrackId(null);
        setActiveView("library");
        showToast(
          library.isRunningBatch
            ? "Tags validés — analyse du lot en cours…"
            : "File de révision terminée — pistes prêtes à exporter",
        );
      }
    },
    [
      library.isRunningBatch,
      library.validateTrackFromReview,
      reviewTracks,
      showToast,
    ],
  );

  const handleTrackResolvedFromReview = useCallback(
    (trackId: string, result: ManualExportResult, localArtworkPath?: string) => {
      const wasInReviewQueue = reviewTracks.some((t) => t.id === trackId);
      const remainingList = reviewTracks.filter((t) => t.id !== trackId);
      library.resolveTrackFromReview(trackId, result, localArtworkPath);

      if (wasInReviewQueue && remainingList.length === 0) {
        setSelectedReviewTrackId(null);
        setActiveView("library");
        showToast(
          library.isRunningBatch
            ? "Export terminé — analyse du lot en cours…"
            : "File de révision terminée — toutes les pistes sont exportées",
        );
      } else if (!wasInReviewQueue) {
        setSelectedReviewTrackId(null);
        setActiveView("library");
        showToast("Export terminé — 1 piste exportée");
      } else {
        setSelectedReviewTrackId(remainingList[0].id);
        showToast("Export terminé — 1 piste exportée");
      }
    },
    [
      library.isRunningBatch,
      library.resolveTrackFromReview,
      reviewTracks,
      showToast,
    ],
  );

  const handleKeepDuplicateTrack = useCallback(
    (_keepId: string, dropId: string, keptLabel: string) => {
      library.removeTracks([dropId]);
      showToast(`${keptLabel} conservée — doublon retiré de la file`);
    },
    [library.removeTracks, showToast],
  );

  const handleKeepBothDuplicates = useCallback(
    (idA: string, idB: string) => {
      library.dismissDuplicatePair(idA, idB);
      showToast("Les deux versions ont été conservées dans la file");
    },
    [library.dismissDuplicatePair, showToast],
  );

  const handleDeleteBothDuplicates = useCallback(
    (idA: string, idB: string) => {
      library.removeTracks([idA, idB]);
      showToast("Les deux pistes ont été retirées de la file");
    },
    [library.removeTracks, showToast],
  );

  return (
    <GlassShell>
      {/* Hidden browser-only fallback inputs (inactive in Tauri desktop runtime) */}
      <input
        ref={library.browserFileInputRef}
        type="file"
        multiple
        accept=".mp3,.m4a,.aac,.flac,.wav"
        className="hidden"
        onChange={(e) => {
          if (e.target.files) library.importBrowserFiles(e.target.files);
          e.target.value = "";
        }}
      />
      <input
        ref={library.browserFolderInputRef}
        type="file"
        multiple
        {...({ webkitdirectory: "" } as Record<string, string>)}
        className="hidden"
        onChange={(e) => {
          if (e.target.files) library.importBrowserFiles(e.target.files);
          e.target.value = "";
        }}
      />

      {/* Full-window Liquid Glass Drag & Drop overlay */}
      <DragOverlay visible={library.isDraggingOver} />

      {/* Left Floating Liquid Glass Sidebar (hidden in Review & Duplicate Focus Modes) */}
      {!isFocusMode && (
        <Sidebar
          activeView={activeView}
          onSelectView={setActiveView}
          trackCount={library.tracks.length}
          reviewCount={library.reviewCount}
          duplicateGroupCount={library.duplicateGroups.length}
          health={health}
          onOpenExportFolder={() => void library.openExportFolder()}
        />
      )}

      {/* Main Column */}
      <main className="flex min-h-0 min-w-0 flex-1 flex-col gap-3.5">
        <TopBar
          activeView={activeView}
          exportMode={exportMode}
          onChangeExportMode={setExportMode}
          theme={theme}
          onToggleTheme={toggleTheme}
          onBackToLibrary={() => {
            setSelectedReviewTrackId(null);
            setSelectedDuplicateGroupKey(null);
            setActiveView("library");
          }}
        />

        {library.errorMessage && (
          <div className="modal-surface text-semantic-danger flex shrink-0 items-center justify-between gap-3 rounded-[16px] border-rose-500/35 px-4 py-2.5 text-[12.5px] font-medium">
            <span className="flex items-center gap-2">
              <AlertCircle className="h-4 w-4 shrink-0" />
              <span>{library.errorMessage}</span>
            </span>
            <button
              type="button"
              onClick={library.clearError}
              aria-label="Fermer l'alerte"
              className="cursor-pointer rounded-full p-1 hover:bg-black/10 dark:hover:bg-white/10"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        )}

        {activeView === "library" && (
          <>
            {library.tracks.length === 0 ? (
              <DropZoneCard
                onPickFolder={() => void library.pickFolder()}
                onPickFiles={() => void library.pickFiles()}
                isScanning={library.isScanning}
              />
            ) : (
              <>
                <TrackQueueList
                  tracks={library.tracks}
                  duplicateGroups={library.duplicateGroups}
                  duplicateGroupByTrackId={library.duplicateGroupByTrackId}
                  onOpenDuplicates={openDuplicateComparator}
                  onRemoveTrack={library.removeTrack}
                  onLookupTrack={(id) => void library.lookupTrack(id)}
                  onOpenReviewTrack={openTrackInReview}
                  onClearAll={library.clearTracks}
                  onPickFolder={() => void library.pickFolder()}
                  onPickFiles={() => void library.pickFiles()}
                  isScanning={library.isScanning}
                  isRunningBatch={library.isRunningBatch}
                  isResumedSession={library.isResumedSession}
                />

                <FloatingActionDock
                  queueStats={library.queueStats}
                  protectedCount={library.protectedCount}
                  totalSizeLabel={library.totalSizeLabel}
                  exportMode={exportMode}
                  hardwareProfile={hardwareProfile}
                  notice={library.notice}
                  isRunningBatch={library.isRunningBatch}
                  batchAction={library.batchAction}
                  batchProgress={library.batchProgress}
                  onRunLookup={() => void library.startBatchLookup()}
                  onRunBatch={() => void handleRunBatchExport()}
                  onForceRunBatch={() =>
                    void handleRunBatchExport({ forceAll: true })
                  }
                  onSelectHardwareProfile={setHardwareProfile}
                  onOpenSettings={() => setActiveView("settings")}
                />
              </>
            )}
          </>
        )}

        {activeView === "review" && (
          <ReviewQueueView
            reviewTracks={editorTracks}
            selectedTrackId={selectedReviewTrackId}
            onSelectTrack={setSelectedReviewTrackId}
            exportMode={exportMode}
            formatMode={formatMode}
            hardwareProfile={hardwareProfile}
            onTrackValidated={handleTrackValidatedFromReview}
            onTrackResolved={handleTrackResolvedFromReview}
            onBackToLibrary={() => {
              setSelectedReviewTrackId(null);
              setActiveView("library");
            }}
          />
        )}

        {activeView === "duplicates" && (
          <DuplicateComparatorView
            groups={library.duplicateGroups}
            tracks={library.tracks}
            selectedGroupKey={selectedDuplicateGroupKey}
            dismissedPairs={library.dismissedDuplicatePairs}
            onSelectGroup={setSelectedDuplicateGroupKey}
            onKeepTrack={handleKeepDuplicateTrack}
            onKeepBoth={handleKeepBothDuplicates}
            onDeleteBoth={handleDeleteBothDuplicates}
            onBackToLibrary={() => {
              setSelectedDuplicateGroupKey(null);
              setActiveView("library");
            }}
          />
        )}

        {activeView === "settings" && (
          <SettingsView
            theme={theme}
            onSelectTheme={setTheme}
            glassTint={glassTint}
            onSelectGlassTint={setGlassTint}
            exportMode={exportMode}
            onSelectExportMode={setExportMode}
            formatMode={formatMode}
            onSelectFormatMode={setFormatMode}
            hardwareProfile={hardwareProfile}
            onSelectHardwareProfile={setHardwareProfile}
          />
        )}
      </main>

      {/* Session Resume Modal (#13) */}
      <SessionResumeModal
        summary={library.pendingSessionSummary}
        onResume={() => void handleResumeSession()}
        onStartFresh={() => void handleStartNewSession()}
      />

      {/* Network Resilience 3-Choice Modal (#13) */}
      <NetworkResilienceModal
        interruption={library.networkInterruption}
        onRetry={() => void handleRetryInterruptedBatch()}
        onExportReady={() => void handleExportReadyFromInterruption()}
        onSaveAndQuit={() => void library.quitAfterSaving()}
        onDismiss={library.dismissNetworkInterruption}
      />

      <ToastNotification
        toast={toast}
        onClose={() => setToast(null)}
        fullWidthCentered={isFocusMode}
      />
    </GlassShell>
  );
}

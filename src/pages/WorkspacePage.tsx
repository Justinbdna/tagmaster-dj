import { useCallback, useMemo, useState } from "react";
import { AlertCircle, X } from "lucide-react";
import { DragOverlay } from "../components/dropzone/DragOverlay";
import { DropZoneCard } from "../components/dropzone/DropZoneCard";
import { GlassShell } from "../components/layout/GlassShell";
import { Sidebar } from "../components/layout/Sidebar";
import { TopBar } from "../components/layout/TopBar";
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
  ExportMode,
  ManualExportResult,
  TrackMetadata,
} from "../types/track";

export function WorkspacePage() {
  const [activeView, setActiveView] = useState<AppView>("library");
  const [exportMode, setExportMode] = useState<ExportMode>("dynamic");
  const [selectedReviewTrackId, setSelectedReviewTrackId] = useState<
    string | null
  >(null);
  const [toast, setToast] = useState<ToastPayload | null>(null);

  const { health } = useAppHealth();
  const { theme, setTheme, toggleTheme, glassTint, setGlassTint } = useTheme();
  const library = useTrackLibrary();

  const showToast = useCallback((message: string) => {
    setToast({ id: Date.now(), message });
  }, []);

  const openTrackInReview = useCallback((trackId: string) => {
    setSelectedReviewTrackId(trackId);
    setActiveView("review");
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
      if (summary && summary.completedCount > 0) {
        showToast(
          `Export terminé — ${summary.completedCount} piste${summary.completedCount > 1 ? "s" : ""} exportée${summary.completedCount > 1 ? "s" : ""}`,
        );
      }
    },
    [exportMode, library.startBatchPipeline, showToast],
  );

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

      {/* Left Floating Liquid Glass Sidebar (hidden in Review Focus Mode) */}
      {activeView !== "review" && (
        <Sidebar
          activeView={activeView}
          onSelectView={setActiveView}
          trackCount={library.tracks.length}
          reviewCount={library.reviewCount}
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
            setActiveView("library");
          }}
        />

        {library.errorMessage && (
          <div className="liquid-glass flex shrink-0 items-center justify-between gap-3 rounded-[16px] border-rose-400/35 px-4 py-2.5 text-[12.5px] text-rose-300">
            <span className="relative z-10 flex items-center gap-2">
              <AlertCircle className="h-4 w-4 shrink-0 text-rose-400" />
              <span>{library.errorMessage}</span>
            </span>
            <button
              type="button"
              onClick={library.clearError}
              aria-label="Fermer l'alerte"
              className="relative z-10 cursor-pointer rounded-full p-1 hover:bg-white/10"
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
                  onRemoveTrack={library.removeTrack}
                  onLookupTrack={(id) => void library.lookupTrack(id)}
                  onOpenReviewTrack={openTrackInReview}
                  onClearAll={library.clearTracks}
                  onPickFolder={() => void library.pickFolder()}
                  onPickFiles={() => void library.pickFiles()}
                  isScanning={library.isScanning}
                  isRunningBatch={library.isRunningBatch}
                />

                <FloatingActionDock
                  queueStats={library.queueStats}
                  protectedCount={library.protectedCount}
                  totalSizeLabel={library.totalSizeLabel}
                  exportMode={exportMode}
                  notice={library.notice}
                  isRunningBatch={library.isRunningBatch}
                  batchAction={library.batchAction}
                  batchProgress={library.batchProgress}
                  onRunLookup={() => void library.startBatchLookup()}
                  onRunBatch={() => void handleRunBatchExport()}
                  onForceRunBatch={() =>
                    void handleRunBatchExport({ forceAll: true })
                  }
                  onOpenExportFolder={() => void library.openExportFolder()}
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
            onTrackValidated={handleTrackValidatedFromReview}
            onTrackResolved={handleTrackResolvedFromReview}
            onBackToLibrary={() => {
              setSelectedReviewTrackId(null);
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
          />
        )}
      </main>

      <ToastNotification
        toast={toast}
        onClose={() => setToast(null)}
        fullWidthCentered={activeView === "review"}
      />
    </GlassShell>
  );
}

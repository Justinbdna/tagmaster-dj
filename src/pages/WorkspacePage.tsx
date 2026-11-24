import { useMemo, useState } from "react";
import { AlertCircle, X } from "lucide-react";
import { DragOverlay } from "../components/dropzone/DragOverlay";
import { DropZoneCard } from "../components/dropzone/DropZoneCard";
import { GlassShell } from "../components/layout/GlassShell";
import { Sidebar } from "../components/layout/Sidebar";
import { TopBar } from "../components/layout/TopBar";
import { FloatingActionDock } from "../components/progress/FloatingActionDock";
import { ReviewPlaceholderView } from "../components/review/ReviewPlaceholderView";
import { SettingsView } from "../components/settings/SettingsView";
import { TrackQueueList } from "../components/tracks/TrackQueueList";
import { useAppHealth } from "../hooks/useAppHealth";
import { useTheme } from "../hooks/useTheme";
import { useTrackLibrary } from "../hooks/useTrackLibrary";
import type { AppView, ExportMode } from "../types/track";

export function WorkspacePage() {
  const [activeView, setActiveView] = useState<AppView>("library");
  const [exportMode, setExportMode] = useState<ExportMode>("dynamic");

  const { health, isTauriRuntime } = useAppHealth();
  const { theme, setTheme, toggleTheme, glassTint, setGlassTint } = useTheme();
  const library = useTrackLibrary();

  const reviewTracks = useMemo(
    () =>
      library.tracks.filter(
        (t) => t.status === "needs_review" || t.status === "failed",
      ),
    [library.tracks],
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

      {/* Left Floating Liquid Glass Sidebar */}
      <Sidebar
        activeView={activeView}
        onSelectView={setActiveView}
        trackCount={library.tracks.length}
        reviewCount={library.reviewCount}
        exportMode={exportMode}
        health={health}
        isTauriRuntime={isTauriRuntime}
        onOpenExportFolder={() => void library.openExportFolder()}
      />

      {/* Main Column */}
      <main className="flex min-h-0 min-w-0 flex-1 flex-col gap-3.5">
        <TopBar
          activeView={activeView}
          trackCount={library.tracks.length}
          totalSizeLabel={library.totalSizeLabel}
          exportMode={exportMode}
          onChangeExportMode={setExportMode}
          theme={theme}
          onToggleTheme={toggleTheme}
          onPickFolder={() => {
            setActiveView("library");
            void library.pickFolder();
          }}
          isScanning={library.isScanning || library.isRunningBatch}
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
                  onClearAll={library.clearTracks}
                  onPickFolder={() => void library.pickFolder()}
                  onPickFiles={() => void library.pickFiles()}
                  isScanning={library.isScanning}
                  isRunningBatch={library.isRunningBatch}
                />

                <FloatingActionDock
                  trackCount={library.tracks.length}
                  protectedCount={library.protectedCount}
                  totalSizeLabel={library.totalSizeLabel}
                  exportMode={exportMode}
                  notice={library.notice}
                  isRunningBatch={library.isRunningBatch}
                  batchProgress={library.batchProgress}
                  lastBatchSummary={library.lastBatchSummary}
                  onRunBatch={() => void library.startBatchPipeline(exportMode)}
                  onOpenExportFolder={() => void library.openExportFolder()}
                />
              </>
            )}
          </>
        )}

        {activeView === "review" && (
          <ReviewPlaceholderView reviewTracks={reviewTracks} />
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
    </GlassShell>
  );
}

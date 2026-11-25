import {
  FolderOpen,
  Loader2,
  RotateCcw,
  Sparkles,
  Upload,
} from "lucide-react";
import type {
  ExportMode,
  PipelineProgressEvent,
  QueueStats,
} from "../../types/track";

interface FloatingActionDockProps {
  queueStats: QueueStats;
  protectedCount: number;
  totalSizeLabel: string;
  exportMode: ExportMode;
  notice: string | null;
  isRunningBatch: boolean;
  batchAction: "lookup" | "export" | null;
  batchProgress: PipelineProgressEvent | null;
  onRunLookup: () => void;
  onRunBatch: () => void;
  onForceRunBatch: () => void;
  onOpenExportFolder: () => void;
}

export function FloatingActionDock({
  queueStats,
  protectedCount,
  totalSizeLabel,
  exportMode,
  notice,
  isRunningBatch,
  batchAction,
  batchProgress,
  onRunLookup,
  onRunBatch,
  onForceRunBatch,
  onOpenExportFolder,
}: FloatingActionDockProps) {
  const resolvedCount =
    queueStats.identifiedCount +
    queueStats.exportedCount +
    queueStats.needsReviewCount +
    queueStats.failedCount;
  const hasAnyProcessed = resolvedCount > 0;
  const isAllExported =
    queueStats.totalCount > 0 &&
    queueStats.exportedCount === queueStats.totalCount &&
    !isRunningBatch;

  const progressPercent = isRunningBatch
    ? (batchProgress?.percent ?? 0)
    : queueStats.totalCount > 0
      ? Math.round((resolvedCount / queueStats.totalCount) * 100)
      : 0;

  let progressLabel: string;
  if (isRunningBatch && batchProgress) {
    const verb = batchAction === "lookup" ? "Identification" : "Export";
    progressLabel = `${verb} (${batchProgress.currentIndex} / ${batchProgress.totalTracks} · ${batchProgress.percent} %) — ${batchProgress.currentFileName}`;
  } else if (isRunningBatch) {
    progressLabel =
      batchAction === "lookup"
        ? "Identification en cours…"
        : "Préparation de l'export…";
  } else if (hasAnyProcessed) {
    const parts: string[] = [];
    if (queueStats.pendingCount > 0) {
      parts.push(`${queueStats.pendingCount} en attente`);
    }
    if (queueStats.identifiedCount > 0) {
      parts.push(
        `${queueStats.identifiedCount} identifiée${queueStats.identifiedCount > 1 ? "s" : ""}`,
      );
    }
    if (queueStats.exportedCount > 0) {
      parts.push(
        `${queueStats.exportedCount} exportée${queueStats.exportedCount > 1 ? "s" : ""}`,
      );
    }
    if (queueStats.needsReviewCount > 0) {
      parts.push(`${queueStats.needsReviewCount} à réviser`);
    }
    if (queueStats.failedCount > 0) {
      parts.push(
        `${queueStats.failedCount} erreur${queueStats.failedCount > 1 ? "s" : ""}`,
      );
    }
    progressLabel = parts.join(" · ");
  } else {
    progressLabel =
      notice ?? "Prêt à identifier puis exporter sans modifier les originaux";
  }

  return (
    <footer className="liquid-glass flex shrink-0 flex-wrap items-center justify-between gap-4 rounded-[22px] px-5 py-3.5">
      {/* Left: Single-source queue metrics & real-time progress bar */}
      <div className="relative z-10 min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2 text-[12.5px] font-medium text-primary">
          <span className="tabular-nums">
            {queueStats.totalCount} piste{queueStats.totalCount > 1 ? "s" : ""}
          </span>
          <span className="text-muted">·</span>
          <span className="tabular-nums text-secondary">{totalSizeLabel}</span>
          <span className="text-muted">·</span>
          <span className="text-secondary">
            Export {exportMode === "dynamic" ? "Dynamique" : "Plat"}
          </span>
          {protectedCount > 0 && (
            <>
              <span className="text-muted">·</span>
              <span className="text-semantic-warning font-semibold">
                {protectedCount} version{protectedCount > 1 ? "s" : ""} protégée
                {protectedCount > 1 ? "s" : ""}
              </span>
            </>
          )}
        </div>

        {/* Real-time progress bar */}
        <div className="mt-2 flex items-center gap-3">
          <div className="h-1.5 min-w-[120px] flex-1 overflow-hidden rounded-full bg-white/10">
            <div
              className={`h-full rounded-full transition-all duration-300 ${
                isAllExported
                  ? "bg-gradient-to-r from-emerald-400 to-sky-400"
                  : "bg-gradient-to-r from-sky-400 to-blue-500"
              }`}
              style={{ width: `${progressPercent}%` }}
            />
          </div>
          <span
            className="max-w-[380px] shrink-0 truncate text-[11px] tabular-nums text-muted"
            title={progressLabel}
          >
            {progressLabel}
          </span>
        </div>
      </div>

      {/* Right: Folder icon + Reset icon + Identifier (read-only) + Exporter */}
      <div className="relative z-10 flex shrink-0 items-center gap-2">
        <button
          type="button"
          onClick={onOpenExportFolder}
          title="Ouvrir le dossier Documents/Tagmaster_DJ_Export"
          aria-label="Ouvrir le dossier d'export"
          className="liquid-glass-pill flex h-9 w-9 cursor-pointer items-center justify-center rounded-full text-secondary hover:text-primary"
        >
          <FolderOpen className="h-4 w-4 text-sky-400" strokeWidth={1.9} />
        </button>

        {!isRunningBatch &&
          (queueStats.exportedCount > 0 ||
            queueStats.needsReviewCount > 0 ||
            queueStats.failedCount > 0) && (
            <button
              type="button"
              onClick={onForceRunBatch}
              title="Tout relancer dans le mode d'export actuel"
              aria-label="Tout relancer"
              className="liquid-glass-pill flex h-9 w-9 cursor-pointer items-center justify-center rounded-full text-secondary hover:text-primary"
            >
              <RotateCcw className="h-3.5 w-3.5" strokeWidth={1.9} />
            </button>
          )}

        <button
          type="button"
          onClick={onRunLookup}
          disabled={isRunningBatch || queueStats.identifiableCount === 0}
          title="Identifier les pistes en attente via Apple Music (lecture seule, sans exporter)"
          className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-4 py-2 text-[12.5px] font-medium text-primary disabled:cursor-not-allowed disabled:opacity-45"
        >
          {isRunningBatch && batchAction === "lookup" ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin text-sky-400" />
          ) : (
            <Sparkles className="h-3.5 w-3.5 text-sky-400" strokeWidth={1.9} />
          )}
          <span>
            {queueStats.identifiableCount > 0
              ? `Identifier (${queueStats.identifiableCount})`
              : "Identifier"}
          </span>
        </button>

        <button
          type="button"
          onClick={onRunBatch}
          disabled={isRunningBatch || queueStats.exportableCount === 0}
          title="Exporter les pistes identifiées (ou identifier puis exporter) vers Tagmaster_DJ_Export"
          className="liquid-glass-primary flex cursor-pointer items-center gap-1.5 rounded-full px-4 py-2 text-[12.5px] font-semibold disabled:cursor-not-allowed disabled:opacity-55"
        >
          {isRunningBatch && batchAction === "export" ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" strokeWidth={2} />
          ) : (
            <Upload className="h-3.5 w-3.5" strokeWidth={2} />
          )}
          <span>
            {queueStats.exportableCount > 0
              ? `Exporter (${queueStats.exportableCount})`
              : "Exporter"}
          </span>
        </button>
      </div>
    </footer>
  );
}

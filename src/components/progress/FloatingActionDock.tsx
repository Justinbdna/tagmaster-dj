import { FolderOpen, Loader2, RotateCcw, Sparkles } from "lucide-react";
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
  batchProgress: PipelineProgressEvent | null;
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
  batchProgress,
  onRunBatch,
  onForceRunBatch,
  onOpenExportFolder,
}: FloatingActionDockProps) {
  const resolvedCount =
    queueStats.exportedCount +
    queueStats.needsReviewCount +
    queueStats.failedCount;
  const hasAnyProcessed = resolvedCount > 0;
  const isAllSettled =
    hasAnyProcessed && queueStats.pendingCount === 0 && !isRunningBatch;

  const progressPercent = isRunningBatch
    ? (batchProgress?.percent ?? 0)
    : queueStats.totalCount > 0
      ? Math.round((resolvedCount / queueStats.totalCount) * 100)
      : 0;

  let progressLabel: string;
  if (isRunningBatch && batchProgress) {
    progressLabel = `En cours (${batchProgress.currentIndex} / ${batchProgress.totalTracks} · ${batchProgress.percent} %) — ${batchProgress.currentFileName}`;
  } else if (isRunningBatch) {
    progressLabel = "Préparation du lot en cours…";
  } else if (hasAnyProcessed) {
    const parts: string[] = [];
    if (queueStats.pendingCount > 0) {
      parts.push(`${queueStats.pendingCount} en attente`);
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
    progressLabel =
      queueStats.pendingCount === 0
        ? `Terminé : ${parts.join(" · ")}`
        : parts.join(" · ");
  } else {
    progressLabel =
      notice ?? "Prêt à nettoyer, tagger et exporter sans modifier les originaux";
  }

  return (
    <footer className="liquid-glass flex shrink-0 flex-wrap items-center justify-between gap-4 rounded-[22px] px-5 py-3.5">
      {/* Left: Queue metrics & real-time progress bar */}
      <div className="relative z-10 min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2 text-[12.5px] font-medium text-primary">
          <span className="tabular-nums">
            {queueStats.totalCount} piste{queueStats.totalCount > 1 ? "s" : ""}{" "}
            dans la file
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
              <span className="text-amber-400">
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
                isAllSettled
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

      {/* Right: Secondary open export / force rerun + Primary launch batch */}
      <div className="relative z-10 flex shrink-0 items-center gap-2.5">
        <button
          type="button"
          onClick={onOpenExportFolder}
          title="Ouvrir le dossier Documents/Tagmaster_DJ_Export"
          className="liquid-glass-pill flex cursor-pointer items-center gap-2 rounded-full px-4 py-2.5 text-[12.5px] font-medium text-primary"
        >
          <FolderOpen className="h-4 w-4 text-sky-400" strokeWidth={1.9} />
          <span>Ouvrir Export</span>
        </button>

        {!isRunningBatch && hasAnyProcessed && (
          <button
            type="button"
            onClick={onForceRunBatch}
            title="Relancer toutes les pistes dans le mode d'export actuel"
            className="liquid-glass-pill flex cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-2.5 text-[12px] font-medium text-primary"
          >
            <RotateCcw className="h-3.5 w-3.5 text-secondary" strokeWidth={1.9} />
            <span>Tout relancer</span>
          </button>
        )}

        <button
          type="button"
          onClick={onRunBatch}
          disabled={isRunningBatch || queueStats.eligibleToExportCount === 0}
          title="Lancer l'identification Apple Music (US / UK / FR), la Table Rase et l'export sécurisé"
          className="liquid-glass-primary flex cursor-pointer items-center gap-2 rounded-full px-5 py-2.5 text-[12.5px] font-semibold disabled:cursor-not-allowed disabled:opacity-60"
        >
          {isRunningBatch ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" strokeWidth={2} />
              <span className="tabular-nums">
                Traitement{" "}
                {batchProgress
                  ? `${batchProgress.currentIndex} / ${batchProgress.totalTracks}`
                  : "…"}
              </span>
            </>
          ) : (
            <>
              <Sparkles className="h-4 w-4" strokeWidth={2} />
              <span>
                {queueStats.eligibleToExportCount > 0 &&
                queueStats.eligibleToExportCount < queueStats.totalCount
                  ? `Taguer & Exporter (${queueStats.eligibleToExportCount})`
                  : "Taguer & Exporter"}
              </span>
            </>
          )}
        </button>
      </div>
    </footer>
  );
}

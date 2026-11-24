import { FolderOpen, Loader2, Sparkles } from "lucide-react";
import type {
  BatchSummary,
  ExportMode,
  PipelineProgressEvent,
} from "../../types/track";

interface FloatingActionDockProps {
  trackCount: number;
  protectedCount: number;
  totalSizeLabel: string;
  exportMode: ExportMode;
  notice: string | null;
  isRunningBatch: boolean;
  batchProgress: PipelineProgressEvent | null;
  lastBatchSummary: BatchSummary | null;
  onRunBatch: () => void;
  onOpenExportFolder: () => void;
}

export function FloatingActionDock({
  trackCount,
  protectedCount,
  totalSizeLabel,
  exportMode,
  notice,
  isRunningBatch,
  batchProgress,
  lastBatchSummary,
  onRunBatch,
  onOpenExportFolder,
}: FloatingActionDockProps) {
  const progressPercent = isRunningBatch
    ? (batchProgress?.percent ?? 0)
    : lastBatchSummary
      ? 100
      : 0;

  let progressLabel: string;
  if (isRunningBatch && batchProgress) {
    progressLabel = `En cours (${batchProgress.currentIndex} / ${batchProgress.totalTracks} · ${batchProgress.percent} %) — ${batchProgress.currentFileName}`;
  } else if (isRunningBatch) {
    progressLabel = "Préparation du lot en cours…";
  } else if (lastBatchSummary) {
    const parts = [
      `${lastBatchSummary.completedCount} exportée${lastBatchSummary.completedCount > 1 ? "s" : ""}`,
    ];
    if (lastBatchSummary.needsReviewCount > 0) {
      parts.push(`${lastBatchSummary.needsReviewCount} à réviser`);
    }
    if (lastBatchSummary.failedCount > 0) {
      parts.push(
        `${lastBatchSummary.failedCount} erreur${lastBatchSummary.failedCount > 1 ? "s" : ""}`,
      );
    }
    progressLabel = `Terminé : ${parts.join(" · ")}`;
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
            {trackCount} piste{trackCount > 1 ? "s" : ""} prête
            {trackCount > 1 ? "s" : ""}
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
                lastBatchSummary && !isRunningBatch
                  ? "bg-gradient-to-r from-emerald-400 to-sky-400"
                  : "bg-gradient-to-r from-sky-400 to-indigo-500"
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

      {/* Right: Secondary open export + Primary launch batch */}
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

        <button
          type="button"
          onClick={onRunBatch}
          disabled={isRunningBatch || trackCount === 0}
          title="Lancer l'identification Apple Music US, la Table Rase et l'export sécurisé"
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
              <span>Taguer &amp; Exporter</span>
            </>
          )}
        </button>
      </div>
    </footer>
  );
}

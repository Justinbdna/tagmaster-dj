import { createPortal } from "react-dom";
import {
  CheckCircle2,
  Clock3,
  FolderTree,
  Layers,
  Play,
  RotateCcw,
  Sparkles,
} from "lucide-react";
import type { SessionCacheSummary } from "../../types/track";

interface SessionResumeModalProps {
  summary: SessionCacheSummary | null;
  onResume: () => void;
  onStartFresh: () => void;
}

function formatSavedTime(timestampMs: number): string {
  if (!timestampMs || !Number.isFinite(timestampMs)) return "";
  const diffSec = Math.max(0, Math.floor((Date.now() - timestampMs) / 1000));
  if (diffSec < 60) return "à l'instant";
  if (diffSec < 3600) {
    const mins = Math.floor(diffSec / 60);
    return `il y a ${mins} min`;
  }
  if (diffSec < 86400) {
    const hrs = Math.floor(diffSec / 3600);
    return `il y a ${hrs} h`;
  }
  return new Date(timestampMs).toLocaleDateString("fr-FR", {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function SessionResumeModal({
  summary,
  onResume,
  onStartFresh,
}: SessionResumeModalProps) {
  if (!summary || !summary.hasSession || !summary.isUnfinished) {
    return null;
  }

  const savedLabel = formatSavedTime(summary.updatedAtMs);

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="session-resume-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 p-4 backdrop-blur-sm"
    >
      <div className="modal-surface relative w-full max-w-lg overflow-hidden rounded-[24px] p-6">
        <div className="flex flex-col gap-5">
          {/* Header */}
          <div className="flex items-center gap-3.5">
            <div className="liquid-glass-primary flex h-11 w-11 shrink-0 items-center justify-center rounded-[14px]">
              <RotateCcw className="h-5 w-5 text-white" strokeWidth={2} />
            </div>
            <div className="min-w-0 flex-1">
              <h2
                id="session-resume-title"
                className="text-[17px] font-semibold tracking-tight text-primary"
              >
                Session précédente non terminée
              </h2>
              <p className="mt-0.5 text-[12.5px] text-secondary">
                {summary.totalTracks} piste{summary.totalTracks > 1 ? "s" : ""}
                {savedLabel ? ` · sauvegardée ${savedLabel}` : ""}
              </p>
            </div>
          </div>

          {/* Metrics grid */}
          <div className="grid grid-cols-3 gap-2.5">
            <div className="modal-subcard rounded-[14px] px-3.5 py-3">
              <div className="text-semantic-success flex items-center gap-1.5 text-[11.5px] font-semibold">
                <CheckCircle2 className="h-3.5 w-3.5" />
                <span>Exportées</span>
              </div>
              <p className="mt-1 text-[20px] font-semibold tabular-nums text-primary">
                {summary.exportedCount}
              </p>
            </div>

            <div className="modal-subcard rounded-[14px] px-3.5 py-3">
              <div className="text-semantic-info flex items-center gap-1.5 text-[11.5px] font-semibold">
                <Sparkles className="h-3.5 w-3.5" />
                <span>Identifiées</span>
              </div>
              <p className="mt-1 text-[20px] font-semibold tabular-nums text-primary">
                {summary.identifiedCount}
              </p>
            </div>

            <div className="modal-subcard rounded-[14px] px-3.5 py-3">
              <div className="text-semantic-warning flex items-center gap-1.5 text-[11.5px] font-semibold">
                <Clock3 className="h-3.5 w-3.5" />
                <span>Restantes</span>
              </div>
              <p className="mt-1 text-[20px] font-semibold tabular-nums text-primary">
                {summary.remainingCount}
              </p>
            </div>
          </div>

          {/* Context strip */}
          <div className="modal-subcard flex flex-wrap items-center justify-between gap-2 rounded-[14px] px-3.5 py-2.5 text-[12px] text-secondary">
            <span>
              {summary.needsReviewCount > 0 ? (
                <span className="text-semantic-warning font-medium">
                  {summary.needsReviewCount} à réviser
                </span>
              ) : (
                <span>Prêt à reprendre là où vous vous êtes arrêté</span>
              )}
            </span>
            <span className="inline-flex items-center gap-1.5 text-[11.5px] font-semibold text-primary">
              {summary.exportMode === "dynamic" ? (
                <FolderTree className="text-semantic-info h-3.5 w-3.5" />
              ) : (
                <Layers className="text-semantic-info h-3.5 w-3.5" />
              )}
              Export {summary.exportMode === "dynamic" ? "Dynamique" : "Plat"}
            </span>
          </div>

          {/* Actions */}
          <div className="flex flex-wrap items-center justify-end gap-2.5 pt-0.5">
            <button
              type="button"
              onClick={onStartFresh}
              className="liquid-glass-pill cursor-pointer rounded-full px-4 py-2 text-[12.5px] font-medium text-primary"
            >
              Nouvelle session
            </button>
            <button
              type="button"
              onClick={onResume}
              className="liquid-glass-primary flex cursor-pointer items-center gap-2 rounded-full px-5 py-2 text-[12.5px] font-semibold"
            >
              <Play className="h-3.5 w-3.5 fill-current" />
              <span>Reprendre la session précédente</span>
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

import { createPortal } from "react-dom";
import {
  CheckCircle2,
  Power,
  RefreshCw,
  Upload,
  WifiOff,
  X,
} from "lucide-react";
import type { NetworkInterruptionState } from "../../types/track";

interface NetworkResilienceModalProps {
  interruption: NetworkInterruptionState | null;
  onRetry: () => void;
  onExportReady: () => void;
  onSaveAndQuit: () => void;
  onDismiss: () => void;
}

function summarizeNetworkError(raw: string): string {
  const lower = raw.toLowerCase();
  if (lower.includes("429") || lower.includes("too many") || lower.includes("rate")) {
    return "Limite de requêtes API atteinte (HTTP 429).";
  }
  if (
    lower.includes("http 500") ||
    lower.includes("http 502") ||
    lower.includes("http 503") ||
    lower.includes("http 504")
  ) {
    return "Serveur temporairement indisponible.";
  }
  if (lower.includes("timed out") || lower.includes("timeout")) {
    return "Délai de connexion dépassé.";
  }
  return "Connexion perdue ou limite API atteinte.";
}

export function NetworkResilienceModal({
  interruption,
  onRetry,
  onExportReady,
  onSaveAndQuit,
  onDismiss,
}: NetworkResilienceModalProps) {
  if (!interruption) return null;

  const readyCount = interruption.readyToExportCount;
  const canExportReady = readyCount > 0;
  const exportReadyLabel =
    readyCount === 0
      ? "Aucune piste prête"
      : readyCount === 1
        ? "Exporter la piste prête (1)"
        : `Exporter les ${readyCount} pistes prêtes`;

  return createPortal(
    <div
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="network-resilience-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 p-4 backdrop-blur-sm"
    >
      <div className="modal-surface relative w-full max-w-lg overflow-hidden rounded-[24px] p-6">
        <div className="flex flex-col gap-4">
          {/* Header */}
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-center gap-3.5">
              <div className="badge-warning flex h-11 w-11 shrink-0 items-center justify-center rounded-[14px] border">
                <WifiOff className="h-5 w-5" strokeWidth={2} />
              </div>
              <div className="min-w-0">
                <h2
                  id="network-resilience-title"
                  className="text-[17px] font-semibold tracking-tight text-primary"
                >
                  Connexion interrompue
                </h2>
                <p className="mt-0.5 text-[12.5px] text-secondary">
                  Traitement mis en pause. Votre progression est sauvegardée.
                </p>
              </div>
            </div>

            <button
              type="button"
              onClick={onDismiss}
              aria-label="Fermer la fenêtre"
              className="flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-full text-secondary transition-colors hover:bg-black/10 hover:text-primary dark:hover:bg-white/10"
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {/* Short diagnostic + progress counts */}
          <div className="modal-subcard flex flex-col gap-2 rounded-[14px] px-4 py-3">
            <p
              className="text-semantic-warning text-[12.5px] font-medium"
              title={interruption.errorMessage}
            >
              {summarizeNetworkError(interruption.errorMessage)}
            </p>
            <div className="flex flex-wrap items-center gap-2.5 text-[12px] text-secondary">
              <span className="text-semantic-success inline-flex items-center gap-1 font-medium">
                <CheckCircle2 className="h-3.5 w-3.5" />
                {interruption.exportedCount} exportée
                {interruption.exportedCount > 1 ? "s" : ""}
              </span>
              <span className="text-muted">·</span>
              <span className="text-semantic-info font-medium">
                {readyCount} prête{readyCount > 1 ? "s" : ""}
              </span>
              <span className="text-muted">·</span>
              <span className="font-medium text-primary">
                {interruption.remainingCount} restante
                {interruption.remainingCount > 1 ? "s" : ""}
              </span>
            </div>
          </div>

          {/* 3 Choices */}
          <div className="flex flex-col gap-2.5 pt-0.5">
            <button
              type="button"
              onClick={onRetry}
              className="liquid-glass-primary flex w-full cursor-pointer items-center justify-center gap-2 rounded-full px-5 py-2.5 text-[13px] font-semibold"
            >
              <RefreshCw className="h-4 w-4" />
              <span>Réessayer</span>
            </button>

            <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-2">
              <button
                type="button"
                onClick={onExportReady}
                disabled={!canExportReady}
                className="btn-semantic-success flex cursor-pointer items-center justify-center gap-2 rounded-full px-4 py-2.5 text-[12.5px] font-semibold disabled:cursor-not-allowed disabled:opacity-45"
              >
                <Upload className="h-3.5 w-3.5" />
                <span>{exportReadyLabel}</span>
              </button>

              <button
                type="button"
                onClick={onSaveAndQuit}
                className="btn-semantic-danger flex cursor-pointer items-center justify-center gap-2 rounded-full px-4 py-2.5 text-[12.5px] font-semibold"
              >
                <Power className="h-3.5 w-3.5" />
                <span>Quitter (sauvegardé)</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

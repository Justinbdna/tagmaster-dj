import { useState } from "react";
import { AlertTriangle, Check, Copy, X } from "lucide-react";
import type { RuntimeDependenciesStatus } from "../../types/track";

interface DependencyBannerProps {
  status: RuntimeDependenciesStatus | null;
  onDismiss: () => void;
}

export function DependencyBanner({
  status,
  onDismiss,
}: DependencyBannerProps) {
  const [copied, setCopied] = useState(false);

  if (!status || status.allAvailable) return null;

  const missing: string[] = [];
  if (!status.ffmpeg.available) missing.push("ffmpeg");
  if (!status.ytdlp.available) missing.push("yt-dlp");

  if (missing.length === 0) return null;

  const installCmd = status.installCommand ?? "";

  const handleCopy = async () => {
    if (!installCmd) return;
    try {
      await navigator.clipboard.writeText(installCmd);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback silencieux si le presse-papiers est indisponible
    }
  };

  return (
    <div
      role="status"
      className="modal-surface flex shrink-0 flex-wrap items-center justify-between gap-2.5 rounded-[16px] px-4 py-2.5 text-[12px]"
    >
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <AlertTriangle className="text-semantic-warning h-4 w-4 shrink-0" />
        <span className="font-medium text-primary">
          {missing.length > 1
            ? `Outils optionnels manquants : ${missing.join(" et ")}`
            : `Outil optionnel manquant : ${missing[0]}`}
        </span>
        {installCmd && (
          <div className="modal-subcard inline-flex items-center gap-1.5 rounded-full px-3 py-1">
            <code className="font-mono text-[11px] font-medium text-primary">
              {installCmd}
            </code>
            <button
              type="button"
              onClick={() => void handleCopy()}
              title="Copier la commande d'installation"
              aria-label="Copier la commande d'installation"
              className="flex h-5 w-5 cursor-pointer items-center justify-center rounded-full text-secondary transition-colors hover:bg-black/10 hover:text-primary dark:hover:bg-white/10"
            >
              {copied ? (
                <Check className="text-semantic-success h-3 w-3" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
            </button>
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={onDismiss}
        aria-label="Fermer l'avis de dépendance"
        className="flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-secondary transition-colors hover:bg-black/10 hover:text-primary dark:hover:bg-white/10"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

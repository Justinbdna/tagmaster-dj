import {
  FileAudio2,
  FolderOpen,
  Music2,
  ShieldCheck,
  Sparkles,
} from "lucide-react";

interface DropZoneCardProps {
  onPickFolder: () => void;
  onPickFiles: () => void;
  isScanning: boolean;
}

const SUPPORTED_FORMATS = ["MP3", "M4A", "AAC", "FLAC", "WAV"];

export function DropZoneCard({
  onPickFolder,
  onPickFiles,
  isScanning,
}: DropZoneCardProps) {
  return (
    <div className="content-surface flex flex-1 flex-col items-center justify-center rounded-[24px] p-8 text-center">
      <div className="flex max-w-lg flex-col items-center">
        {/* Liquid Glass Concentric Medallion */}
        <div className="liquid-glass mb-6 flex h-20 w-20 items-center justify-center rounded-[24px]">
          <div className="relative z-10 flex h-12 w-12 items-center justify-center rounded-[14px] bg-gradient-to-br from-sky-400/25 to-indigo-500/25 text-sky-400">
            <Music2 className="h-6 w-6" strokeWidth={1.85} />
          </div>
        </div>

        <h3 className="text-[22px] font-semibold tracking-tight text-primary">
          Déposez vos dossiers ou morceaux ici
        </h3>

        <p className="mt-2.5 text-[13.5px] leading-relaxed text-secondary">
          Glissez un dossier complet (scan récursif automatique) ou sélectionnez
          vos pistes audio pour lancer le nettoyage « Table Rase » et
          l&apos;enrichissement Apple Music (US / UK / FR).
        </p>

        {/* Supported Formats */}
        <div className="mt-5 flex flex-wrap items-center justify-center gap-1.5">
          {SUPPORTED_FORMATS.map((fmt) => (
            <span
              key={fmt}
              className="rounded-full border border-[var(--content-border)] bg-[var(--content-card-bg)] px-2.5 py-1 text-[10.5px] font-semibold tracking-wider text-secondary"
            >
              .{fmt}
            </span>
          ))}
        </div>

        {/* Action Buttons */}
        <div className="mt-7 flex flex-wrap items-center justify-center gap-3">
          <button
            type="button"
            onClick={onPickFolder}
            disabled={isScanning}
            className="liquid-glass-primary flex cursor-pointer items-center gap-2 rounded-full px-5 py-2.5 text-[13px] font-semibold disabled:cursor-wait disabled:opacity-60"
          >
            <FolderOpen className="h-4 w-4" strokeWidth={2} />
            <span>
              {isScanning ? "Analyse du dossier…" : "Sélectionner un dossier"}
            </span>
          </button>

          <button
            type="button"
            onClick={onPickFiles}
            disabled={isScanning}
            className="liquid-glass-pill flex cursor-pointer items-center gap-2 rounded-full px-5 py-2.5 text-[13px] font-medium text-primary disabled:cursor-wait disabled:opacity-60"
          >
            <FileAudio2 className="h-4 w-4 text-sky-400" strokeWidth={1.9} />
            <span>Choisir des fichiers</span>
          </button>
        </div>

        {/* Local-first & Non-destructive Guarantees */}
        <div className="mt-8 flex flex-wrap items-center justify-center gap-4 text-[11.5px] text-muted">
          <span className="inline-flex items-center gap-1.5">
            <ShieldCheck className="h-3.5 w-3.5 text-emerald-400" />
            Originaux jamais modifiés
          </span>
          <span className="inline-flex items-center gap-1.5">
            <Sparkles className="h-3.5 w-3.5 text-sky-400" />
            Détection automatique Live / Remix / Sped Up
          </span>
        </div>
      </div>
    </div>
  );
}

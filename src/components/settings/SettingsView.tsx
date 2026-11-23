import {
  FolderTree,
  KeyRound,
  Layers,
  Moon,
  ShieldCheck,
  Sparkles,
  Sun,
} from "lucide-react";
import type { ExportMode, GlassTint, ThemeMode } from "../../types/track";

interface SettingsViewProps {
  theme: ThemeMode;
  onSelectTheme: (theme: ThemeMode) => void;
  glassTint: GlassTint;
  onSelectGlassTint: (tint: GlassTint) => void;
  exportMode: ExportMode;
  onSelectExportMode: (mode: ExportMode) => void;
}

export function SettingsView({
  theme,
  onSelectTheme,
  glassTint,
  onSelectGlassTint,
  exportMode,
  onSelectExportMode,
}: SettingsViewProps) {
  return (
    <section className="content-surface custom-scrollbar flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto rounded-[24px] p-6">
      {/* Appearance Section */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-5">
        <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
          <Sparkles className="h-4 w-4 text-sky-400" />
          <h3>Apparence Liquid Glass (iOS 26 / 27)</h3>
        </div>
        <p className="mt-1 text-[12.5px] text-secondary">
          Mode sombre activé par défaut avec option clair et réglage de densité
          du verre pour un contraste optimal.
        </p>

        <div className="mt-4 grid gap-4 md:grid-cols-2">
          {/* Theme */}
          <div className="flex flex-col gap-2">
            <span className="text-[11.5px] font-semibold uppercase tracking-wider text-muted">
              Thème
            </span>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => onSelectTheme("dark")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[14px] px-4 py-3 text-[13px] font-medium transition-all ${
                  theme === "dark"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <Moon className="h-4 w-4 text-sky-400" />
                <span>Sombre</span>
              </button>
              <button
                type="button"
                onClick={() => onSelectTheme("light")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[14px] px-4 py-3 text-[13px] font-medium transition-all ${
                  theme === "light"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <Sun className="h-4 w-4 text-amber-400" />
                <span>Clair</span>
              </button>
            </div>
          </div>

          {/* Glass Density */}
          <div className="flex flex-col gap-2">
            <span className="text-[11.5px] font-semibold uppercase tracking-wider text-muted">
              Densité du verre
            </span>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => onSelectGlassTint("clear")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[14px] px-4 py-3 text-[13px] font-medium transition-all ${
                  glassTint === "clear"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <span>Translucide</span>
              </button>
              <button
                type="button"
                onClick={() => onSelectGlassTint("tinted")}
                className={`flex cursor-pointer items-center justify-center gap-2 rounded-[14px] px-4 py-3 text-[13px] font-medium transition-all ${
                  glassTint === "tinted"
                    ? "liquid-glass-active text-primary"
                    : "border border-[var(--content-border)] text-secondary hover:text-primary"
                }`}
              >
                <span>Teinté (contraste +)</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Export Architecture Section */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-5">
        <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
          <ShieldCheck className="h-4 w-4 text-emerald-400" />
          <h3>Arborescence d&apos;export sécurisée</h3>
        </div>
        <p className="mt-1 text-[12.5px] text-secondary">
          Tous les fichiers traités sont copiés vers{" "}
          <code className="text-sky-400">Documents/Tagmaster_DJ_Export</code>{" "}
          sans jamais toucher aux originaux.
        </p>

        <div className="mt-4 grid gap-3 md:grid-cols-2">
          <button
            type="button"
            onClick={() => onSelectExportMode("dynamic")}
            className={`flex cursor-pointer flex-col items-start rounded-[16px] p-4 text-left transition-all ${
              exportMode === "dynamic"
                ? "liquid-glass-active"
                : "border border-[var(--content-border)] hover:bg-[var(--content-card-hover)]"
            }`}
          >
            <div className="flex items-center gap-2 text-[13px] font-semibold text-primary">
              <FolderTree className="h-4 w-4 text-sky-400" />
              <span>Mode Dynamique</span>
            </div>
            <p className="mt-1.5 font-mono text-[11px] text-secondary">
              Export / Artiste / Album / Artiste - Titre.mp3
            </p>
          </button>

          <button
            type="button"
            onClick={() => onSelectExportMode("flat")}
            className={`flex cursor-pointer flex-col items-start rounded-[16px] p-4 text-left transition-all ${
              exportMode === "flat"
                ? "liquid-glass-active"
                : "border border-[var(--content-border)] hover:bg-[var(--content-card-hover)]"
            }`}
          >
            <div className="flex items-center gap-2 text-[13px] font-semibold text-primary">
              <Layers className="h-4 w-4 text-indigo-400" />
              <span>Mode Plat</span>
            </div>
            <p className="mt-1.5 font-mono text-[11px] text-secondary">
              Export / Artiste - Titre.mp3
            </p>
          </button>
        </div>
      </div>

      {/* API Keys local-only note (Sprint 2) */}
      <div className="rounded-[18px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-5">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[14px] font-semibold text-primary">
            <KeyRound className="h-4 w-4 text-amber-400" />
            <h3>Clés API optionnelles (.env local)</h3>
          </div>
          <span className="rounded-full bg-white/[0.08] px-2.5 py-0.5 text-[10.5px] font-semibold text-muted">
            Sprint 2
          </span>
        </div>
        <p className="mt-1.5 text-[12.5px] leading-relaxed text-secondary">
          Apple Music US fonctionne sans clé API dès le Sprint 1. La gestion des
          clés optionnelles <code className="text-primary">GENIUS_API_KEY</code>{" "}
          (paroles) et <code className="text-primary">DISCOGS_TOKEN</code>{" "}
          (éditions CD) stockées exclusivement dans votre fichier{" "}
          <code className="text-primary">.env</code> local sera activée au
          Sprint 2.
        </p>
      </div>
    </section>
  );
}

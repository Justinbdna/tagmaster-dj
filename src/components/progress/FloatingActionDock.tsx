import { Sparkles } from "lucide-react";
import type { ExportMode } from "../../types/track";

interface FloatingActionDockProps {
  trackCount: number;
  protectedCount: number;
  totalSizeLabel: string;
  exportMode: ExportMode;
  notice: string | null;
}

export function FloatingActionDock({
  trackCount,
  protectedCount,
  totalSizeLabel,
  exportMode,
  notice,
}: FloatingActionDockProps) {
  return (
    <footer className="liquid-glass flex shrink-0 flex-wrap items-center justify-between gap-4 rounded-[22px] px-5 py-3.5">
      {/* Left: Queue metrics & last scan notice */}
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

        {/* Progress track placeholder ready for Sprint 1 Issue #5 */}
        <div className="mt-2 flex items-center gap-3">
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-white/10">
            <div className="h-full w-0 rounded-full bg-gradient-to-r from-sky-400 to-indigo-500 transition-all duration-300" />
          </div>
          <span className="shrink-0 text-[11px] text-muted">
            {notice ?? "0 % · En attente du pipeline Apple Music US (Bloc 2)"}
          </span>
        </div>
      </div>

      {/* Right: Pipeline launch button (wired in Sprint 1 Bloc 2 & 3) */}
      <div className="relative z-10 shrink-0">
        <button
          type="button"
          disabled
          title="Le moteur Apple Music US + Tagging ID3 sera connecté au Bloc 2 (Issues #3 & #4)"
          className="liquid-glass-primary flex cursor-not-allowed items-center gap-2 rounded-full px-5 py-2.5 text-[12.5px] font-semibold opacity-65"
        >
          <Sparkles className="h-4 w-4" strokeWidth={2} />
          <span>Taguer &amp; Exporter (Bloc 2)</span>
        </button>
      </div>
    </footer>
  );
}

import { ArrowLeft, FolderTree, Layers, Moon, Sun } from "lucide-react";
import type { AppView, ExportMode, ThemeMode } from "../../types/track";

interface TopBarProps {
  activeView: AppView;
  exportMode: ExportMode;
  onChangeExportMode: (mode: ExportMode) => void;
  theme: ThemeMode;
  onToggleTheme: () => void;
  onBackToLibrary?: () => void;
}

const VIEW_TITLES: Record<AppView, { title: string; subtitle: string }> = {
  library: {
    title: "Bibliothèque Audio",
    subtitle: "Glissez vos dossiers ou morceaux pour préparer le tagging",
  },
  review: {
    title: "File de Révision",
    subtitle: "Édition manuelle des titres ambigus et versions alternatives",
  },
  settings: {
    title: "Paramètres",
    subtitle: "Apparence Liquid Glass, mode d'export et clés API locales",
  },
};

export function TopBar({
  activeView,
  exportMode,
  onChangeExportMode,
  theme,
  onToggleTheme,
  onBackToLibrary,
}: TopBarProps) {
  const meta = VIEW_TITLES[activeView];

  return (
    <header className="liquid-glass flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-[22px] px-5 py-3.5">
      {/* Left: Focus back button (in review mode) + Title & subtitle */}
      <div className="relative z-10 flex min-w-0 items-center gap-3.5">
        {activeView === "review" && onBackToLibrary && (
          <button
            type="button"
            onClick={onBackToLibrary}
            className="liquid-glass-pill flex shrink-0 cursor-pointer items-center gap-1.5 rounded-full px-3.5 py-1.5 text-[12px] font-medium text-primary"
          >
            <ArrowLeft className="h-3.5 w-3.5 text-sky-400" />
            <span>Retour à la Bibliothèque</span>
          </button>
        )}

        <div className="min-w-0">
          <h2 className="truncate text-[17px] font-semibold tracking-tight text-primary">
            {meta.title}
          </h2>
          <p className="truncate text-[12px] text-secondary">{meta.subtitle}</p>
        </div>
      </div>

      {/* Interactive Liquid Glass Controls */}
      <div className="relative z-10 flex flex-wrap items-center gap-2.5">
        {/* Segmented Control: Flat vs Dynamic Export Mode */}
        <div
          className="liquid-glass-pill flex items-center rounded-full p-1"
          role="group"
          aria-label="Mode d'arborescence d'export"
        >
          <button
            type="button"
            onClick={() => onChangeExportMode("dynamic")}
            title="Export / Artiste / Album / Artiste - Titre"
            className={`flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[12px] font-medium transition-all duration-200 ${
              exportMode === "dynamic"
                ? "liquid-glass-active text-primary"
                : "text-secondary hover:text-primary"
            }`}
          >
            <FolderTree className="h-3.5 w-3.5" strokeWidth={1.9} />
            <span>Dynamique</span>
          </button>

          <button
            type="button"
            onClick={() => onChangeExportMode("flat")}
            title="Export / Artiste - Titre (tout dans un seul dossier)"
            className={`flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[12px] font-medium transition-all duration-200 ${
              exportMode === "flat"
                ? "liquid-glass-active text-primary"
                : "text-secondary hover:text-primary"
            }`}
          >
            <Layers className="h-3.5 w-3.5" strokeWidth={1.9} />
            <span>Plat</span>
          </button>
        </div>

        {/* Dark / Light Theme Switcher */}
        <button
          type="button"
          onClick={onToggleTheme}
          title={
            theme === "dark"
              ? "Passer en mode clair"
              : "Passer en mode sombre"
          }
          aria-label="Basculer le thème clair/sombre"
          className="liquid-glass-pill flex h-9 w-9 cursor-pointer items-center justify-center rounded-full text-secondary hover:text-primary"
        >
          {theme === "dark" ? (
            <Sun className="h-4 w-4 text-amber-300" strokeWidth={1.9} />
          ) : (
            <Moon className="h-4 w-4 text-indigo-500" strokeWidth={1.9} />
          )}
        </button>
      </div>
    </header>
  );
}

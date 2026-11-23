import {
  Disc3,
  FolderOutput,
  Library,
  ListChecks,
  Settings,
  ShieldCheck,
} from "lucide-react";
import type { AppView, ExportMode, HealthStatus } from "../../types/track";

interface SidebarProps {
  activeView: AppView;
  onSelectView: (view: AppView) => void;
  trackCount: number;
  reviewCount: number;
  exportMode: ExportMode;
  health: HealthStatus | null;
  isTauriRuntime: boolean;
}

export function Sidebar({
  activeView,
  onSelectView,
  trackCount,
  reviewCount,
  exportMode,
  health,
  isTauriRuntime,
}: SidebarProps) {
  const navItems: {
    id: AppView;
    label: string;
    icon: typeof Library;
    badge?: string | number;
    muted?: boolean;
  }[] = [
    {
      id: "library",
      label: "Bibliothèque",
      icon: Library,
      badge: trackCount > 0 ? trackCount : undefined,
    },
    {
      id: "review",
      label: "File de révision",
      icon: ListChecks,
      badge: reviewCount > 0 ? reviewCount : "S2",
      muted: reviewCount === 0,
    },
    {
      id: "settings",
      label: "Paramètres",
      icon: Settings,
    },
  ];

  return (
    <aside className="liquid-glass flex h-full w-[252px] shrink-0 flex-col justify-between rounded-[24px] p-3.5">
      {/* Top: Brand + Navigation */}
      <div className="relative z-10 flex flex-col gap-5">
        <div className="flex items-center gap-3 px-1.5 pt-1">
          <div className="liquid-glass-primary flex h-10 w-10 shrink-0 items-center justify-center rounded-[14px]">
            <Disc3 className="h-5 w-5 text-white" strokeWidth={1.9} />
          </div>
          <div className="min-w-0">
            <h1 className="truncate text-[15px] font-semibold tracking-tight text-primary">
              Tagmaster DJ
            </h1>
            <p className="truncate text-[11px] font-medium text-muted">
              Local-First · v{health?.version ?? "0.1.0"}
            </p>
          </div>
        </div>

        <nav className="flex flex-col gap-1" aria-label="Navigation principale">
          <span className="px-2.5 pb-1 text-[10px] font-semibold uppercase tracking-[0.14em] text-muted">
            Espace de travail
          </span>
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeView === item.id;
            return (
              <button
                key={item.id}
                type="button"
                onClick={() => onSelectView(item.id)}
                className={`group flex w-full cursor-pointer items-center justify-between rounded-[14px] px-3 py-2.5 text-left text-[13px] font-medium transition-all duration-200 ${
                  isActive
                    ? "liquid-glass-active text-primary"
                    : "border border-transparent text-secondary hover:bg-white/[0.06] hover:text-primary"
                }`}
              >
                <span className="flex items-center gap-2.5">
                  <Icon
                    className={`h-[17px] w-[17px] transition-colors ${
                      isActive ? "text-sky-400" : "text-muted group-hover:text-secondary"
                    }`}
                    strokeWidth={1.85}
                  />
                  <span>{item.label}</span>
                </span>

                {item.badge !== undefined && (
                  <span
                    className={`rounded-full px-2 py-0.5 text-[10px] font-semibold tabular-nums ${
                      item.muted
                        ? "bg-white/[0.07] text-muted"
                        : "bg-sky-500/20 text-sky-400"
                    }`}
                  >
                    {item.badge}
                  </span>
                )}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Bottom: Export destination safety card + Runtime health */}
      <div className="relative z-10 flex flex-col gap-2.5">
        <div className="rounded-[16px] border border-[var(--content-border)] bg-[var(--content-card-bg)] p-3">
          <div className="flex items-center gap-2 text-[11px] font-semibold text-primary">
            <FolderOutput className="h-3.5 w-3.5 text-sky-400" strokeWidth={2} />
            <span>Destination sécurisée</span>
          </div>
          <p className="mt-1.5 truncate font-mono text-[10.5px] text-secondary">
            Documents/Tagmaster_DJ_Export
          </p>
          <div className="mt-2 flex items-center gap-1.5 text-[10.5px] text-muted">
            <ShieldCheck className="h-3.5 w-3.5 shrink-0 text-emerald-400" />
            <span>
              Mode {exportMode === "dynamic" ? "Dynamique" : "Plat"} · Originaux
              intacts
            </span>
          </div>
        </div>

        <div className="liquid-glass-pill flex items-center justify-between rounded-full px-3.5 py-2 text-[11px] font-medium">
          <span className="flex items-center gap-2 text-secondary">
            <span
              className={`h-2 w-2 rounded-full ${
                isTauriRuntime
                  ? "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.7)]"
                  : "bg-amber-400"
              }`}
            />
            {isTauriRuntime ? "Moteur Rust actif" : "Mode aperçu Web"}
          </span>
          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">
            {isTauriRuntime ? "Tauri 2" : "Vite"}
          </span>
        </div>
      </div>
    </aside>
  );
}

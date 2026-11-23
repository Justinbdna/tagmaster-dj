import type { ReactNode } from "react";

interface GlassShellProps {
  children: ReactNode;
}

/**
 * Conteneur principal iOS 26 / macOS Tahoe :
 * Toile ambiante vivante en arrière-plan pour nourrir la réfraction du Liquid Glass.
 */
export function GlassShell({ children }: GlassShellProps) {
  return (
    <div className="ambient-canvas relative h-screen w-screen overflow-hidden p-3.5 text-primary select-none">
      <div className="relative z-10 flex h-full w-full gap-3.5">{children}</div>
    </div>
  );
}

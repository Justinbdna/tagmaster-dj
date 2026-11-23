import type { ReactNode } from "react";

interface GlassShellProps {
  children: ReactNode;
}

export function GlassShell({ children }: GlassShellProps) {
  return (
    <div className="min-h-screen w-full px-6 py-8 text-white/95 flex items-center justify-center">
      <div className="glass-panel w-full max-w-5xl rounded-[28px] p-8">
        {children}
      </div>
    </div>
  );
}

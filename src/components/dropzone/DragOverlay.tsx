import { FolderDown } from "lucide-react";

interface DragOverlayProps {
  visible: boolean;
}

export function DragOverlay({ visible }: DragOverlayProps) {
  if (!visible) return null;

  return (
    <div
      className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-6 backdrop-blur-md transition-opacity duration-200"
      aria-live="polite"
    >
      <div className="liquid-glass flex max-w-md flex-col items-center rounded-[30px] border-[var(--glass-border-strong)] px-10 py-9 text-center shadow-[0_0_48px_var(--accent-glow)]">
        <div className="liquid-glass-primary mb-4 flex h-16 w-16 items-center justify-center rounded-[20px]">
          <FolderDown className="h-8 w-8 text-white" strokeWidth={1.9} />
        </div>
        <h3 className="text-[19px] font-semibold tracking-tight text-primary">
          Relâchez pour importer
        </h3>
        <p className="mt-1.5 text-[13px] text-secondary">
          Dossiers et fichiers audio (.mp3, .m4a, .aac, .flac, .wav) seront
          scannés automatiquement.
        </p>
      </div>
    </div>
  );
}

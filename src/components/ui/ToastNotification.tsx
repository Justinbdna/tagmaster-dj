import { useEffect } from "react";
import { createPortal } from "react-dom";
import { Check, X } from "lucide-react";

export interface ToastPayload {
  id: number;
  message: string;
}

interface ToastNotificationProps {
  toast: ToastPayload | null;
  onClose: () => void;
  durationMs?: number;
}

export function ToastNotification({
  toast,
  onClose,
  durationMs = 3500,
}: ToastNotificationProps) {
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => {
      onClose();
    }, durationMs);
    return () => {
      window.clearTimeout(timer);
    };
  }, [toast, durationMs, onClose]);

  if (!toast || typeof document === "undefined") return null;

  return createPortal(
    <div
      role="status"
      aria-live="polite"
      className="pointer-events-none fixed right-5 bottom-5 z-50"
    >
      <div className="liquid-glass pointer-events-auto flex max-w-sm items-center gap-2.5 rounded-full px-4 py-2.5">
        <Check
          className="relative z-10 h-4 w-4 shrink-0 text-emerald-400"
          strokeWidth={2.3}
        />
        <span className="relative z-10 truncate text-[12.5px] font-medium text-primary">
          {toast.message}
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="Fermer la notification"
          className="relative z-10 flex h-5 w-5 shrink-0 cursor-pointer items-center justify-center rounded-full text-muted transition-colors hover:bg-white/10 hover:text-primary"
        >
          <X className="h-3 w-3" />
        </button>
      </div>
    </div>,
    document.body,
  );
}

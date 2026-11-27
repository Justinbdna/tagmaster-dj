import { useEffect } from "react";
import { createPortal } from "react-dom";
import { Check, X } from "lucide-react";

export interface ToastPayload {
  id: number;
  message: string;
  actionLabel?: string;
  onAction?: () => void;
}

interface ToastNotificationProps {
  toast: ToastPayload | null;
  onClose: () => void;
  durationMs?: number;
  fullWidthCentered?: boolean;
}

export function ToastNotification({
  toast,
  onClose,
  durationMs = 3500,
  fullWidthCentered = false,
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
      className={`pointer-events-none fixed top-[49px] left-1/2 z-50 -translate-x-1/2 -translate-y-1/2 ${
        fullWidthCentered ? "" : "md:left-[calc(50%+127px)]"
      }`}
    >
      <div className="modal-surface pointer-events-auto flex max-w-xs items-center gap-2 rounded-full px-3.5 py-1.5 sm:max-w-md">
        <Check
          className="text-semantic-success h-3.5 w-3.5 shrink-0"
          strokeWidth={2.4}
        />
        <span className="truncate text-[12px] font-medium text-primary">
          {toast.message}
        </span>
        {toast.actionLabel && toast.onAction && (
          <button
            type="button"
            onClick={() => {
              toast.onAction?.();
              onClose();
            }}
            className="text-semantic-info shrink-0 cursor-pointer rounded-full bg-sky-500/15 px-2.5 py-0.5 text-[11px] font-semibold transition-colors hover:bg-sky-500/25"
          >
            {toast.actionLabel}
          </button>
        )}
        <button
          type="button"
          onClick={onClose}
          aria-label="Fermer la notification"
          className="flex h-5 w-5 shrink-0 cursor-pointer items-center justify-center rounded-full text-secondary transition-colors hover:bg-black/10 hover:text-primary dark:hover:bg-white/10"
        >
          <X className="h-3 w-3" />
        </button>
      </div>
    </div>,
    document.body,
  );
}

import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "@phosphor-icons/react";
import { cn } from "@/lib/cn";

/** Modal that becomes a bottom sheet on phones. Escape + backdrop close, focus is trapped loosely. */
export function Dialog({ open, onClose, title, children, className, dismissable = true }: { open: boolean; onClose: () => void; title: string; children: ReactNode; className?: string; dismissable?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && dismissable && onClose();
    document.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";
    requestAnimationFrame(() => ref.current?.focus());
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
      prev?.focus?.();
    };
  }, [open, onClose, dismissable]);
  if (!open) return null;
  return createPortal(
    <div className="fixed inset-0 z-[100] flex items-end justify-center sm:items-center sm:p-6">
      <div className="absolute inset-0 bg-ink/45 backdrop-blur-[2px] animate-[rise_0.2s_ease-out_both]" onClick={() => dismissable && onClose()} aria-hidden="true" />
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
        className={cn(
          "relative max-h-[92dvh] w-full overflow-y-auto rounded-t-[1.75rem] bg-surface p-6 pb-[max(1.5rem,env(safe-area-inset-bottom))] shadow-float outline-none sm:max-w-md sm:rounded-[1.75rem] animate-rise",
          className,
        )}
      >
        <div className="mb-5 flex items-start justify-between gap-4">
          <h2 className="text-xl font-semibold tracking-tight">{title}</h2>
          {dismissable && (
            <button onClick={onClose} className="-m-2 grid size-10 place-items-center rounded-full text-muted hover:bg-sunk hover:text-ink" aria-label="Close">
              <X size={20} aria-hidden="true" />
            </button>
          )}
        </div>
        {children}
      </div>
    </div>,
    document.body,
  );
}

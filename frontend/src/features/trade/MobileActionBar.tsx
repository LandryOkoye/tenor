import { useEffect, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

/**
 * Phone-only sticky bar above the bottom nav. Hidden while the real buy panel
 * is on screen, so it never duplicates the visible CTA.
 */
export function MobileActionBar({ target, children }: { target: RefObject<HTMLElement | null>; children: ReactNode }) {
  const [panelVisible, setPanelVisible] = useState(true);
  // Reserve room at the page bottom so the bar never covers the footer.
  useEffect(() => {
    document.body.style.paddingBottom = "76px";
    return () => {
      document.body.style.paddingBottom = "";
    };
  }, []);
  useEffect(() => {
    const el = target.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver(([e]) => setPanelVisible(e.isIntersecting), { threshold: 0.15 });
    io.observe(el);
    return () => io.disconnect();
  }, [target]);
  return createPortal(
    <div
      className={`fixed inset-x-0 bottom-[calc(4rem+env(safe-area-inset-bottom))] z-30 border-t border-line bg-paper/95 px-4 py-3 backdrop-blur-md transition-[transform,opacity] duration-200 md:bottom-0 lg:hidden ${panelVisible ? "pointer-events-none translate-y-4 opacity-0" : "opacity-100"}`}
      aria-hidden={panelVisible}
    >
      {children}
    </div>,
    document.body,
  );
}

export function scrollToPanel(el: HTMLElement | null) {
  el?.scrollIntoView({ behavior: "smooth", block: "start" });
}

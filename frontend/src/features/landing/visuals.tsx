import { useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "@/lib/cn";

/** Fades/rises children in once they scroll into view. */
export function Reveal({ children, className, delay = 0 }: { children: ReactNode; className?: string; delay?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof IntersectionObserver === "undefined") return setShown(true);
    const io = new IntersectionObserver(([e]) => e.isIntersecting && (setShown(true), io.disconnect()), { rootMargin: "0px 0px -10% 0px" });
    io.observe(el);
    return () => io.disconnect();
  }, []);
  return (
    <div
      ref={ref}
      className={cn("transition-[opacity,transform] duration-700 ease-[cubic-bezier(0.2,0.7,0.2,1)]", shown ? "opacity-100 translate-y-0" : "opacity-0 translate-y-6", className)}
      style={{ transitionDelay: `${delay}ms` }}
    >
      {children}
    </div>
  );
}

/** Elongated dark pill used inline in the giant headline (Charter motif). */
export function HeroPill({ children, className }: { children?: ReactNode; className?: string }) {
  return (
    <span className={cn("relative inline-block h-[0.62em] w-[1.25em] translate-y-[-0.04em] rounded-full bg-ink align-middle shadow-[inset_0_-6px_14px_rgb(255_255_255/0.08)]", className)} aria-hidden="true">
      {children}
    </span>
  );
}

/** Small floating white tile with a soft shadow. */
export function Tile({ children, className, rotate = 0, delay = 0, desktopOnly = false }: { children: ReactNode; className?: string; rotate?: number; delay?: number; desktopOnly?: boolean }) {
  return (
    <span
      className={cn("absolute place-items-center", desktopOnly ? "hidden sm:grid" : "grid", "rounded-[22%] bg-surface shadow-float animate-float", className)}
      style={{ ["--r" as string]: `${rotate}deg`, animationDelay: `${delay}ms` }}
      aria-hidden="true"
    >
      {children}
    </span>
  );
}

/** Large converge-lines artwork for dark panels: many legs flowing into one dot. */
export function ConvergeArt({ className, legs = 3, animated = true }: { className?: string; legs?: number; animated?: boolean }) {
  const ys = Array.from({ length: legs }, (_, i) => 30 + (i * 240) / Math.max(1, legs - 1));
  return (
    <svg viewBox="0 0 400 300" className={className} aria-hidden="true" preserveAspectRatio="xMaxYMid slice">
      <defs>
        <radialGradient id="glow" cx="78%" cy="50%" r="40%">
          <stop offset="0" stopColor="#ff6b1a" stopOpacity="0.55" />
          <stop offset="1" stopColor="#ff6b1a" stopOpacity="0" />
        </radialGradient>
      </defs>
      <rect width="400" height="300" fill="url(#glow)" />
      {ys.map((y, i) => (
        <g key={i}>
          <path d={`M-10 ${y} C 140 ${y}, 200 150, 290 150`} stroke="rgb(250 250 248 / 0.18)" strokeWidth="10" fill="none" strokeLinecap="round" />
          <path
            d={`M-10 ${y} C 140 ${y}, 200 150, 290 150`}
            stroke="rgb(250 250 248 / 0.9)"
            strokeWidth="2"
            fill="none"
            strokeDasharray="6 14"
            className={animated ? "animate-flow" : undefined}
            style={{ animationDelay: `${i * -0.6}s` }}
          />
        </g>
      ))}
      <circle cx="312" cy="150" r="22" fill="#ff6b1a" />
    </svg>
  );
}

/** Phone frame for product mocks; content is real UI, not a screenshot. */
export function Phone({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn("relative w-[290px] rounded-[3rem] bg-ink p-[10px] shadow-[0_40px_80px_-30px_rgb(10_10_10/0.55)]", className)} aria-hidden="true">
      <div className="relative h-[590px] overflow-hidden rounded-[2.4rem] bg-paper text-ink">
        <div className="absolute left-1/2 top-2.5 z-10 h-6 w-24 -translate-x-1/2 rounded-full bg-ink" />
        <div className="flex items-center justify-between px-7 pt-3.5 text-[11px] font-semibold num">
          <span>9:41</span>
          <span className="flex gap-1">
            <span className="h-2.5 w-4 rounded-sm bg-ink" />
          </span>
        </div>
        {children}
      </div>
    </div>
  );
}

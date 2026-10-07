import { cn } from "@/lib/cn";

/** The Converge mark: three basket legs meeting one resolved outcome. */
export function Mark({ className, inverted = false, animated = false }: { className?: string; inverted?: boolean; animated?: boolean }) {
  const stroke = inverted ? "var(--color-paper)" : "var(--color-ink)";
  return (
    <svg viewBox="0 0 120 100" className={cn("shrink-0", className)} aria-hidden="true">
      <g stroke={stroke} strokeWidth="11" strokeLinecap="round" fill="none">
        <path d="M10 14 L68 42" />
        <path d="M10 50 H66" />
        <path d="M10 86 L68 58" />
      </g>
      <circle cx="94" cy="50" r="17" fill="var(--color-accent)" className={animated ? "origin-[94px_50px] animate-pulse-dot" : undefined} />
    </svg>
  );
}

export function Logo({ className, inverted = false }: { className?: string; inverted?: boolean }) {
  return (
    <span className={cn("inline-flex items-center gap-2", className)}>
      <Mark className="h-[0.95em] w-auto" inverted={inverted} />
      <span className={cn("font-display font-bold tracking-[-0.06em] leading-none", inverted ? "text-paper" : "text-ink")}>tenor</span>
    </span>
  );
}

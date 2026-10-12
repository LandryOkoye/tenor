import { type ReactNode } from "react";
import { WarningCircle, ArrowClockwise } from "@phosphor-icons/react";
import { cn } from "@/lib/cn";
import { PANTA_URL } from "@/lib/env";
import { formatPrice } from "@/lib/money";
import type { Side } from "@/api/types";

export function Card({ className, children, as: As = "div" }: { className?: string; children: ReactNode; as?: "div" | "section" | "article" }) {
  return <As className={cn("rounded-[var(--radius-card)] border border-line bg-surface", className)}>{children}</As>;
}

export function Badge({ tone = "neutral", className, children }: { tone?: "neutral" | "yes" | "no" | "accent" | "warn" | "dark"; className?: string; children: ReactNode }) {
  const tones = {
    neutral: "bg-sunk text-muted",
    yes: "bg-yes-soft text-yes",
    no: "bg-no-soft text-no",
    accent: "bg-accent-soft text-accent-ink",
    warn: "bg-warn-soft text-warn",
    dark: "bg-ink text-paper",
  } as const;
  return <span className={cn("inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium whitespace-nowrap", tones[tone], className)}>{children}</span>;
}

export function SideBadge({ side, className }: { side: Side; className?: string }) {
  return (
    <Badge tone={side === "yes" ? "yes" : "no"} className={cn("font-mono uppercase tracking-wider text-[11px]", className)}>
      {side}
    </Badge>
  );
}

/** "62¢ · 62% chance": brief §12. */
export function PriceTag({ price, side, size = "md" }: { price: string | null; side?: Side; size?: "sm" | "md" | "lg" }) {
  const p = formatPrice(price);
  return (
    <span className="inline-flex items-baseline gap-1.5 num">
      <span className={cn("font-display font-semibold tracking-tight", size === "lg" ? "text-3xl" : size === "md" ? "text-lg" : "text-sm", side === "yes" && "text-yes", side === "no" && "text-no")}>
        {p.cents}
      </span>
      <span className={cn("text-muted", size === "sm" ? "text-xs" : "text-sm")}>{p.pct === "–" ? "" : `${p.pct} chance`}</span>
    </span>
  );
}

/** Two-tone YES/NO probability bar. */
export function OddsBar({ yes, className }: { yes: string | null; className?: string }) {
  const v = yes == null ? null : Math.min(1, Math.max(0, Number(yes)));
  return (
    <div className={cn("flex h-1.5 w-full overflow-hidden rounded-full bg-sunk", className)} role="img" aria-label={v == null ? "No price yet" : `Yes ${Math.round(v * 100)} percent`}>
      {v != null && (
        <>
          <span className="bg-yes" style={{ width: `${v * 100}%` }} />
          <span className="bg-no/70" style={{ width: `${(1 - v) * 100}%` }} />
        </>
      )}
    </div>
  );
}

export function PoweredByPanta({ className, inverted }: { className?: string; inverted?: boolean }) {
  return (
    <a
      href={PANTA_URL}
      target="_blank"
      rel="noreferrer"
      className={cn("inline-flex items-center gap-1.5 text-xs font-medium hover:underline underline-offset-4", inverted ? "text-paper/70" : "text-muted", className)}
    >
      <span className="size-1.5 rounded-full bg-accent" aria-hidden="true" />
      Powered by Panta
    </a>
  );
}

export function Skeleton({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded-xl bg-sunk", className)} aria-hidden="true" />;
}

export function EmptyState({ icon, title, body, action, className }: { icon?: ReactNode; title: string; body?: ReactNode; action?: ReactNode; className?: string }) {
  return (
    <div className={cn("flex flex-col items-center rounded-[var(--radius-card)] border border-dashed border-line-strong bg-surface/60 px-6 py-14 text-center", className)}>
      {icon && <div className="mb-4 grid size-12 place-items-center rounded-2xl bg-sunk text-ink">{icon}</div>}
      <h3 className="text-lg font-semibold tracking-tight">{title}</h3>
      {body && <p className="mt-1.5 max-w-sm text-sm text-muted">{body}</p>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}

export function ErrorState({ error, onRetry, className }: { error: unknown; onRetry?: () => void; className?: string }) {
  const msg = error instanceof Error ? error.message : "Something went wrong.";
  return (
    <div role="alert" className={cn("flex flex-col items-start gap-3 rounded-2xl border border-no/20 bg-no-soft/60 p-5 sm:flex-row sm:items-center", className)}>
      <WarningCircle size={22} className="shrink-0 text-no" aria-hidden="true" />
      <p className="flex-1 text-sm text-ink">{msg}</p>
      {onRetry && (
        <button onClick={onRetry} className="inline-flex items-center gap-1.5 rounded-full bg-surface px-3.5 py-2 text-sm font-medium border border-line hover:bg-sunk">
          <ArrowClockwise size={16} aria-hidden="true" /> Try again
        </button>
      )}
    </div>
  );
}

/** Brief §4.4: cached data must be labelled as such. */
export function StaleNote({ stale, className }: { stale: boolean; className?: string }) {
  if (!stale) return null;
  return (
    <Badge tone="warn" className={className}>
      <WarningCircle size={13} weight="bold" aria-hidden="true" /> Cached price: Panta unreachable
    </Badge>
  );
}

export function SectionHeading({ eyebrow, title, action, className }: { eyebrow?: string; title: ReactNode; action?: ReactNode; className?: string }) {
  return (
    <div className={cn("flex flex-wrap items-end justify-between gap-4", className)}>
      <div>
        {eyebrow && <p className="label-mono text-muted mb-2">{eyebrow}</p>}
        <h2 className="text-2xl font-semibold tracking-[-0.03em] sm:text-3xl">{title}</h2>
      </div>
      {action}
    </div>
  );
}

export function Tabs<T extends string>({ value, onChange, items, className, label }: { value: T; onChange: (v: T) => void; items: Array<{ value: T; label: ReactNode }>; className?: string; label: string }) {
  return (
    <div role="group" aria-label={label} className={cn("no-scrollbar flex gap-1 overflow-x-auto rounded-full border border-line bg-surface p-1", className)}>
      {items.map((it) => (
        <button
          key={it.value}
          type="button"
          aria-pressed={value === it.value}
          onClick={() => onChange(it.value)}
          className={cn(
            "h-9 shrink-0 flex-1 whitespace-nowrap rounded-full px-3 text-[13px] font-medium transition-colors sm:flex-none sm:px-4 sm:text-sm",
            value === it.value ? "bg-ink text-paper" : "text-muted hover:text-ink hover:bg-sunk",
          )}
        >
          {it.label}
        </button>
      ))}
    </div>
  );
}

export function Stat({ label, value, sub, className }: { label: string; value: ReactNode; sub?: ReactNode; className?: string }) {
  return (
    <div className={className}>
      <p className="label-mono text-muted">{label}</p>
      <p className="mt-1.5 font-display text-xl font-semibold tracking-tight num">{value}</p>
      {sub && <p className="mt-0.5 text-xs text-muted">{sub}</p>}
    </div>
  );
}

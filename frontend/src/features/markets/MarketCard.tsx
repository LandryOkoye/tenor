import { Link } from "react-router";
import { Basket, Clock } from "@phosphor-icons/react";
import type { MarketSummary } from "@/api/types";
import { formatPrice, formatUsdcCompact } from "@/lib/money";
import { relative } from "@/lib/time";
import { OddsBar, StaleNote } from "@/components/ui/primitives";

export function MarketCard({ m, inBaskets = 0 }: { m: MarketSummary; inBaskets?: number }) {
  const yes = formatPrice(m.yes_price);
  const no = formatPrice(m.no_price);
  return (
    <Link
      to={`/markets/${encodeURIComponent(m.id)}`}
      className="group flex h-full flex-col rounded-3xl border border-line bg-surface p-5 transition-[border-color,box-shadow,transform] duration-200 hover:-translate-y-0.5 hover:border-line-strong hover:shadow-soft"
    >
      <div className="flex items-center justify-between gap-2">
        <span className="label-mono text-muted">{m.category ?? "Market"}</span>
        {inBaskets > 0 && (
          <span className="inline-flex items-center gap-1 text-xs text-accent-ink">
            <Basket size={14} aria-hidden="true" /> In {inBaskets} basket{inBaskets > 1 ? "s" : ""}
          </span>
        )}
      </div>
      <h3 className="mt-3 flex-1 text-[17px] font-semibold leading-snug tracking-tight group-hover:underline group-hover:decoration-line-strong group-hover:underline-offset-4">{m.title}</h3>
      <div className="mt-5 flex items-end justify-between">
        <div>
          <p className="font-display text-3xl font-semibold tracking-tight num">{yes.pct}</p>
          <p className="text-xs text-muted">chance of YES</p>
        </div>
        <div className="flex gap-1.5 text-sm font-semibold num">
          <span className="rounded-lg bg-yes-soft px-2.5 py-1.5 text-yes">Yes {yes.cents}</span>
          <span className="rounded-lg bg-no-soft px-2.5 py-1.5 text-no">No {no.cents}</span>
        </div>
      </div>
      <OddsBar yes={m.yes_price} className="mt-4" />
      <div className="mt-3 flex items-center justify-between text-xs text-muted num">
        <span>{m.volume_usdc ? `${formatUsdcCompact(m.volume_usdc)} vol` : "–"}</span>
        <span className="inline-flex items-center gap-1">
          <Clock size={13} aria-hidden="true" /> Closes {relative(m.close_time)}
        </span>
      </div>
      {m.stale && <StaleNote stale className="mt-3 self-start" />}
    </Link>
  );
}

export function MarketCardSkeleton() {
  return (
    <div className="h-[232px] rounded-3xl border border-line bg-surface p-5">
      <div className="h-3 w-20 animate-pulse rounded bg-sunk" />
      <div className="mt-4 h-5 w-4/5 animate-pulse rounded bg-sunk" />
      <div className="mt-2 h-5 w-3/5 animate-pulse rounded bg-sunk" />
      <div className="mt-10 h-8 w-16 animate-pulse rounded bg-sunk" />
      <div className="mt-4 h-1.5 w-full animate-pulse rounded bg-sunk" />
    </div>
  );
}

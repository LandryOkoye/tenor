import { Link } from "react-router";
import { ArrowUpRight, Info } from "@phosphor-icons/react";
import type { CreatorScorecard, DataSource, ResolutionMarketRow, ResolutionStatus } from "@/api/types";
import { Badge } from "@/components/ui/primitives";
import { formatPrice, formatUsdcCompact } from "@/lib/money";
import { formatDate, relative } from "@/lib/time";
import { shortAddress } from "@/wallet/useTenorWallet";

export const STATUS: Record<ResolutionStatus, { label: string; tone: "neutral" | "yes" | "warn" | "accent" }> = {
  open: { label: "Open", tone: "neutral" },
  resolving_soon: { label: "Resolving soon", tone: "accent" },
  resolved: { label: "Resolved", tone: "yes" },
  delayed: { label: "Delayed", tone: "warn" },
};

/** Brief §8: everything we compute is labelled "Tenor analysis", separate from Panta data. */
export function SourceTag({ source }: { source: DataSource }) {
  return source === "app_analysis" ? (
    <span className="label-mono inline-flex items-center gap-1 text-accent-ink" title="Computed by Tenor from Panta data. Not an official Panta resolution.">
      <Info size={12} aria-hidden="true" /> Tenor analysis
    </span>
  ) : (
    <span className="label-mono text-muted">Panta data</span>
  );
}

export function ResolutionRow({ r }: { r: ResolutionMarketRow }) {
  const s = STATUS[r.status];
  const surprise = r.outcome && r.last_price_before_resolution && Number(r.last_price_before_resolution) < 0.1;
  return (
    <Link to={`/resolution/markets/${encodeURIComponent(r.market_id)}`} className="group flex flex-col gap-3 px-5 py-4 transition-colors hover:bg-sunk/60 sm:flex-row sm:items-center">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <Badge tone={s.tone}>{s.label}</Badge>
          {r.outcome && <Badge tone={r.outcome === "yes" ? "yes" : "no"} className="font-mono uppercase">{r.outcome}</Badge>}
          {surprise && <Badge tone="warn">Surprise</Badge>}
          <SourceTag source={r.source} />
        </div>
        <p className="mt-2 font-medium leading-snug group-hover:underline underline-offset-4">{r.title}</p>
      </div>
      <div className="grid grid-cols-2 gap-x-6 text-sm sm:w-80 sm:text-right num">
        <div>
          <p className="text-xs text-muted">{r.resolved_at ? "Resolved" : "Expected"}</p>
          <p>{r.resolved_at ? formatDate(r.resolved_at, true) : relative(r.expected_resolve_time)}</p>
        </div>
        <div>
          <p className="text-xs text-muted">{r.last_price_before_resolution ? "Last price" : "Creator"}</p>
          <p className="font-mono text-xs leading-5">{r.last_price_before_resolution ? formatPrice(r.last_price_before_resolution).cents : shortAddress(r.creator)}</p>
        </div>
      </div>
      <ArrowUpRight size={16} className="hidden shrink-0 text-muted sm:block" aria-hidden="true" />
    </Link>
  );
}

export function Scorecard({ s, link = true }: { s: CreatorScorecard; link?: boolean }) {
  const pct = s.pct_resolved_on_time != null ? Math.round(Number(s.pct_resolved_on_time) * 100) : null;
  return (
    <div className="rounded-[var(--radius-card)] bg-ink p-6 text-paper">
      <div className="flex items-center justify-between gap-3">
        <p className="label-mono text-paper/50">Creator track record</p>
        <span className="label-mono text-accent">Tenor analysis</span>
      </div>
      <p className="mt-2 font-mono text-sm break-all">
        {link ? <Link to={`/resolution/creators/${s.creator}`} className="underline underline-offset-4">{shortAddress(s.creator, 8)}</Link> : s.creator}
      </p>
      <div className="mt-6 grid grid-cols-2 gap-3 sm:grid-cols-3">
        {[
          [pct != null ? `${pct}%` : "–", "resolved on time"],
          [s.median_delay_hours != null ? `${Number(s.median_delay_hours).toFixed(1)}h` : "–", "median delay"],
          [String(s.markets_resolved), `of ${String(s.markets_created)} resolved`],
          [String(s.surprise_resolutions), "surprise outcomes"],
          [formatUsdcCompact(s.total_volume_usdc), "total volume"],
        ].map(([v, l]) => (
          <div key={l} className="rounded-2xl bg-paper/[0.07] p-4">
            <p className="font-display text-2xl font-semibold tracking-tight num">{v}</p>
            <p className="text-xs text-paper/55">{l}</p>
          </div>
        ))}
      </div>
      {pct != null && (
        <div className="mt-5 h-1.5 overflow-hidden rounded-full bg-paper/10" role="img" aria-label={`${pct}% resolved on time`}>
          <div className="h-full rounded-full bg-accent" style={{ width: `${pct}%` }} />
        </div>
      )}
      <p className="mt-4 text-xs text-paper/50">
        On time = resolved within 24h of the expected time. Surprise = YES last traded at 90¢ or more, then resolved NO. Computed by Tenor from Panta data. Tenor can't change or overturn a resolution.
      </p>
    </div>
  );
}

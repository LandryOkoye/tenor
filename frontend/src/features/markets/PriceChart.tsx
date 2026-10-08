import { useMemo, useRef, useState } from "react";
import { ChartLine } from "@phosphor-icons/react";
import { useHistory } from "@/api/queries";
import { Skeleton, Tabs, ErrorState } from "@/components/ui/primitives";
import { formatDate } from "@/lib/time";

const RANGES = [
  { value: "1d", label: "1D" },
  { value: "7d", label: "1W" },
  { value: "all", label: "All" },
] as const;
type Range = (typeof RANGES)[number]["value"];

/** YES price history from Tenor's own snapshots (brief D10). Lightweight SVG, no chart lib. */
export function PriceChart({ marketId }: { marketId: string }) {
  const [range, setRange] = useState<Range>("7d");
  const { data, isLoading, isError, error, refetch } = useHistory(marketId, range);
  const [hover, setHover] = useState<number | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  const W = 640, H = 220, P = 8;

  const pts = useMemo(() => (data?.points ?? []).map((p) => ({ t: new Date(p.ts).getTime(), y: Number(p.yes_price) })), [data]);
  const geo = useMemo(() => {
    if (pts.length < 2) return null;
    const t0 = pts[0].t, t1 = pts[pts.length - 1].t;
    const ys = pts.map((p) => p.y);
    const lo = Math.max(0, Math.min(...ys) - 0.05), hi = Math.min(1, Math.max(...ys) + 0.05);
    const x = (t: number) => P + ((t - t0) / Math.max(1, t1 - t0)) * (W - 2 * P);
    const y = (v: number) => P + (1 - (v - lo) / Math.max(0.01, hi - lo)) * (H - 2 * P);
    const line = pts.map((p, i) => `${i ? "L" : "M"}${x(p.t).toFixed(1)},${y(p.y).toFixed(1)}`).join("");
    return { x, y, line, area: `${line}L${x(t1)},${H}L${x(t0)},${H}Z`, lo, hi };
  }, [pts]);

  const onMove = (e: React.PointerEvent) => {
    if (!geo || !svg.current) return;
    const r = svg.current.getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    let best = 0;
    for (let i = 1; i < pts.length; i++) if (Math.abs(geo.x(pts[i].t) - px) < Math.abs(geo.x(pts[best].t) - px)) best = i;
    setHover(best);
  };

  const cur = hover != null ? pts[hover] : pts[pts.length - 1];
  const first = pts[0];
  const change = cur && first ? (cur.y - first.y) * 100 : 0;

  return (
    <div>
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div aria-live="polite">
          {cur ? (
            <>
              <p className="font-display text-4xl font-semibold tracking-tight num">
                {Math.round(cur.y * 100)}% <span className="text-base font-medium text-muted">YES</span>
              </p>
              <p className={`text-sm num ${change >= 0 ? "text-yes" : "text-no"}`}>
                {change >= 0 ? "▲" : "▼"} {Math.abs(change).toFixed(1)} pts · {hover != null ? formatDate(new Date(cur.t).toISOString(), true) : RANGES.find((r) => r.value === range)!.label}
              </p>
            </>
          ) : (
            <p className="text-sm text-muted">Price history</p>
          )}
        </div>
        <Tabs label="Chart range" value={range} onChange={setRange} items={RANGES.map((r) => ({ value: r.value, label: r.label }))} />
      </div>
      <div className="mt-4">
        {isLoading ? (
          <Skeleton className="h-[220px] w-full" />
        ) : isError ? (
          <ErrorState error={error} onRetry={() => refetch()} />
        ) : !geo ? (
          <div className="grid h-[220px] place-items-center rounded-2xl border border-dashed border-line-strong text-center">
            <div>
              <ChartLine size={24} className="mx-auto text-muted" aria-hidden="true" />
              <p className="mt-2 text-sm font-medium">No price history yet</p>
              <p className="text-xs text-muted">History shows from when Tenor started tracking this market.</p>
            </div>
          </div>
        ) : (
          <svg
            ref={svg}
            viewBox={`0 0 ${W} ${H}`}
            className="h-[220px] w-full touch-none"
            preserveAspectRatio="none"
            onPointerMove={onMove}
            onPointerLeave={() => setHover(null)}
            role="img"
            aria-label={`YES price chart, from ${Math.round(first.y * 100)}% to ${Math.round(pts[pts.length - 1].y * 100)}%`}
          >
            <defs>
              <linearGradient id="area" x1="0" x2="0" y1="0" y2="1">
                <stop offset="0" stopColor="var(--color-accent)" stopOpacity="0.22" />
                <stop offset="1" stopColor="var(--color-accent)" stopOpacity="0" />
              </linearGradient>
            </defs>
            {[0.25, 0.5, 0.75].map((f) => (
              <line key={f} x1={0} x2={W} y1={H * f} y2={H * f} stroke="var(--color-line)" strokeDasharray="3 5" vectorEffect="non-scaling-stroke" />
            ))}
            <path d={geo.area} fill="url(#area)" />
            <path d={geo.line} fill="none" stroke="var(--color-ink)" strokeWidth="2" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
            {cur && (
              <>
                {hover != null && <line x1={geo.x(cur.t)} x2={geo.x(cur.t)} y1={0} y2={H} stroke="var(--color-line-strong)" vectorEffect="non-scaling-stroke" />}
                <circle cx={geo.x(cur.t)} cy={geo.y(cur.y)} r="5" fill="var(--color-accent)" stroke="white" strokeWidth="2" vectorEffect="non-scaling-stroke" />
              </>
            )}
          </svg>
        )}
      </div>
      <p className="mt-2 text-xs text-faint">Tenor snapshots of Panta spot prices. Bonding-curve prices aren't pure probabilities.</p>
    </div>
  );
}

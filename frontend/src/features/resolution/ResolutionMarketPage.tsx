import { Link, useParams } from "react-router";
import { ArrowLeft, ArrowSquareOut } from "@phosphor-icons/react";
import { useResolutionMarket } from "@/api/queries";
import { Badge, Card, ErrorState, Skeleton, Stat } from "@/components/ui/primitives";
import { formatPrice } from "@/lib/money";
import { formatDate, relative } from "@/lib/time";
import { cn } from "@/lib/cn";
import { Scorecard, SourceTag, STATUS } from "./parts";

export default function ResolutionMarketPage() {
  const { id = "" } = useParams();
  const { data: d, isLoading, isError, error, refetch } = useResolutionMarket(id);
  if (isError) return <ErrorState error={error} onRetry={() => refetch()} />;
  if (isLoading || !d) return <Skeleton className="h-96" />;
  const s = STATUS[d.status];

  return (
    <div>
      <Link to="/resolution" className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-ink">
        <ArrowLeft size={16} aria-hidden="true" /> Resolution Center
      </Link>
      <div className="mt-5 flex flex-wrap items-center gap-2">
        <Badge tone={s.tone}>{s.label}</Badge>
        {d.outcome && <Badge tone={d.outcome === "yes" ? "yes" : "no"} className="font-mono uppercase">Outcome: {d.outcome}</Badge>}
      </div>
      <h1 className="mt-3 max-w-3xl text-3xl font-semibold leading-tight tracking-[-0.035em] sm:text-[2.6rem]">{d.title}</h1>
      <Link to={`/markets/${encodeURIComponent(d.market_id)}`} className="mt-3 inline-flex text-sm font-medium underline underline-offset-4">Open market page</Link>

      <div className="mt-8 grid gap-6 lg:grid-cols-[1fr_400px]">
        <div className="min-w-0 space-y-6">
          <div className="grid grid-cols-2 gap-4 rounded-3xl border border-line bg-surface p-5 sm:grid-cols-3">
            <Stat label="Expected" value={relative(d.expected_resolve_time)} sub={formatDate(d.expected_resolve_time, true)} />
            <Stat label="Resolved" value={d.resolved_at ? relative(d.resolved_at) : "Not yet"} sub={d.resolved_at ? formatDate(d.resolved_at, true) : undefined} />
            <Stat label="Last price" value={formatPrice(d.last_price_before_resolution).cents} sub="winning side, before resolution" />
          </div>

          <Card className="p-5 sm:p-7">
            <h2 className="text-xl font-semibold tracking-tight">Evidence timeline</h2>
            {d.timeline.length === 0 ? (
              <p className="mt-3 text-sm text-muted">No events recorded yet. Tenor records every state change from when it started tracking this market.</p>
            ) : (
              <ol className="relative mt-6 space-y-6 border-l-2 border-line pl-6">
                {d.timeline.map((e, i) => {
                  const last = i === d.timeline.length - 1;
                  return (
                    <li key={`${e.event_type}-${e.ts}`} className="relative">
                      <span className={cn("absolute -left-[33px] top-1 size-4 rounded-full border-[3px] border-surface", last ? "bg-accent" : "bg-ink")} aria-hidden="true" />
                      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
                        <p className="font-medium">{e.label}</p>
                        <SourceTag source={e.source} />
                      </div>
                      <p className="mt-0.5 text-sm text-muted num">{formatDate(e.ts, true)}</p>
                      {e.explorer_url && (
                        <a href={e.explorer_url} target="_blank" rel="noreferrer" className="mt-1 inline-flex items-center gap-1 text-xs font-medium underline underline-offset-4">
                          On-chain record <ArrowSquareOut size={12} aria-hidden="true" />
                        </a>
                      )}
                    </li>
                  );
                })}
              </ol>
            )}
          </Card>

          <Card className="p-5 sm:p-7">
            <h2 className="text-xl font-semibold tracking-tight">Resolution rules</h2>
            <p className="mt-3 whitespace-pre-line leading-relaxed text-ink/85">{d.rules_text ?? "No rules text published."}</p>
            <div className="mt-5 border-t border-line pt-4 text-sm">
              <p className="label-mono text-muted">Source / oracle</p>
              <p className="mt-1">{d.oracle ?? "Not specified"}</p>
            </div>
          </Card>
        </div>
        <aside className="space-y-4">
          {d.creator_scorecard ? <Scorecard s={d.creator_scorecard} /> : <Card className="p-6 text-sm text-muted">No creator record yet.</Card>}
        </aside>
      </div>
    </div>
  );
}

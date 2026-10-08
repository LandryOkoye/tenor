import { Link, useParams } from "react-router";
import { ArrowLeft, ArrowRight, Basket, Scales, ShareNetwork } from "@phosphor-icons/react";
import { useRef, useState } from "react";
import type { Side } from "@/api/types";
import { useMediaQuery } from "@/lib/useMediaQuery";
import { formatPrice } from "@/lib/money";
import { MobileActionBar, scrollToPanel } from "@/features/trade/MobileActionBar";
import { useMarket } from "@/api/queries";
import { ApiError } from "@/api/client";
import { Card, ErrorState, Skeleton, StaleNote, Stat } from "@/components/ui/primitives";
import { formatUsdcCompact } from "@/lib/money";
import { formatDate, relative } from "@/lib/time";
import { shortAddress } from "@/wallet/useTenorWallet";
import NotFound from "@/components/layout/NotFound";
import { PriceChart } from "./PriceChart";
import { TradeTape } from "./TradeTape";
import { BuyPanel } from "./BuyPanel";

export default function MarketDetailPage() {
  const { id = "" } = useParams();
  const { data: m, isLoading, isError, error, refetch } = useMarket(id);
  const [copied, setCopied] = useState(false);
  const desktop = useMediaQuery("(min-width: 1024px)");
  const panelRef = useRef<HTMLDivElement>(null);
  const [req, setReq] = useState<{ side: Side; n: number }>();

  if (isError && error instanceof ApiError && error.status === 404) return <NotFound />;
  if (isError) return <ErrorState error={error} onRetry={() => refetch()} />;
  if (isLoading || !m) {
    return (
      <div className="grid gap-8 lg:grid-cols-[1fr_380px]">
        <div className="space-y-4">
          <Skeleton className="h-6 w-32" />
          <Skeleton className="h-12 w-3/4" />
          <Skeleton className="h-72" />
        </div>
        <Skeleton className="h-[460px]" />
      </div>
    );
  }

  const share = async () => {
    const url = window.location.href;
    try {
      if (navigator.share) await navigator.share({ title: m.title, url });
      else {
        await navigator.clipboard.writeText(url);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      }
    } catch {
      /* cancelled */
    }
  };

  return (
    <div>
      <Link to="/markets" className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-ink">
        <ArrowLeft size={16} aria-hidden="true" /> Markets
      </Link>

      <div className="mt-5 grid gap-8 lg:grid-cols-[1fr_380px] lg:gap-10">
        <div className="min-w-0 space-y-6">
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="label-mono text-muted">{m.category ?? "Market"}</span>
              <StaleNote stale={m.stale} />
            </div>
            <div className="mt-3 flex items-start justify-between gap-4">
              <h1 className="text-3xl font-semibold leading-tight tracking-[-0.035em] sm:text-[2.6rem]">{m.title}</h1>
              <button onClick={share} className="grid size-11 shrink-0 place-items-center rounded-full border border-line bg-surface hover:border-line-strong" aria-label="Share market">
                <ShareNetwork size={18} aria-hidden="true" />
              </button>
            </div>
            {copied && <p className="mt-1 text-xs text-yes" role="status">Link copied</p>}
            <div className="mt-6 grid grid-cols-3 gap-4 rounded-3xl border border-line bg-surface p-5">
              <Stat label="Volume" value={m.volume_usdc ? formatUsdcCompact(m.volume_usdc) : "–"} />
              <Stat label="Closes" value={relative(m.close_time)} sub={formatDate(m.close_time)} />
              <Stat label="Resolves" value={relative(m.resolve_time)} sub={formatDate(m.resolve_time)} />
            </div>
          </div>

          <Card className="p-5 sm:p-6">
            <PriceChart marketId={m.id} />
          </Card>

          {!desktop && (
            <div ref={panelRef} className="scroll-mt-24">
              <BuyPanel market={m} requestedSide={req} />
            </div>
          )}

          {m.in_baskets.length > 0 && (
            <Card className="p-5 sm:p-6">
              <h2 className="flex items-center gap-2 text-lg font-semibold tracking-tight">
                <Basket size={20} aria-hidden="true" /> In these baskets
              </h2>
              <ul className="mt-3 divide-y divide-line">
                {m.in_baskets.map((b) => (
                  <li key={b.slug}>
                    <Link to={`/baskets/${b.slug}`} className="flex items-center justify-between py-3 font-medium hover:text-accent-ink">
                      {b.title} <ArrowRight size={16} aria-hidden="true" />
                    </Link>
                  </li>
                ))}
              </ul>
            </Card>
          )}

          <Card className="p-5 sm:p-6">
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-lg font-semibold tracking-tight">Rules</h2>
              <Link to={`/resolution/markets/${encodeURIComponent(m.id)}`} className="inline-flex items-center gap-1.5 text-sm font-medium text-muted hover:text-ink">
                <Scales size={16} aria-hidden="true" /> Resolution details
              </Link>
            </div>
            <p className="mt-3 whitespace-pre-line leading-relaxed text-ink/85">{m.rules_text ?? "The creator didn't publish rules text for this market."}</p>
            <dl className="mt-5 grid gap-4 border-t border-line pt-5 text-sm sm:grid-cols-2">
              <div>
                <dt className="label-mono text-muted">Resolution source</dt>
                <dd className="mt-1">{m.oracle ?? "Not specified"}</dd>
              </div>
              <div>
                <dt className="label-mono text-muted">Creator</dt>
                <dd className="mt-1">
                  {m.creator ? (
                    <Link to={`/resolution/creators/${m.creator}`} className="font-mono underline underline-offset-4">{shortAddress(m.creator, 6)}</Link>
                  ) : (
                    "–"
                  )}
                </dd>
              </div>
            </dl>
          </Card>

          <Card className="p-5 sm:p-6">
            <h2 className="text-lg font-semibold tracking-tight">Recent trades</h2>
            <div className="mt-3">
              <TradeTape marketId={m.id} />
            </div>
          </Card>
        </div>

        {desktop && (
          <aside>
            <div className="sticky top-24">
              <BuyPanel market={m} />
            </div>
          </aside>
        )}
      </div>
      {!desktop && (
        <MobileActionBar target={panelRef}>
          <div className="grid grid-cols-2 gap-2">
            {(["yes", "no"] as const).map((s) => (
              <button
                key={s}
                onClick={() => { setReq({ side: s, n: Date.now() }); scrollToPanel(panelRef.current); }}
                className={`h-12 rounded-full text-[15px] font-semibold text-white num ${s === "yes" ? "bg-yes" : "bg-no"}`}
              >
                Buy {s.toUpperCase()} {formatPrice(s === "yes" ? m.yes_price : m.no_price).cents}
              </button>
            ))}
          </div>
        </MobileActionBar>
      )}
    </div>
  );
}

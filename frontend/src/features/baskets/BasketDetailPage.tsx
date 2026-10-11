import { useRef } from "react";
import { Link, useParams } from "react-router";
import { useMediaQuery } from "@/lib/useMediaQuery";
import { MobileActionBar, scrollToPanel } from "@/features/trade/MobileActionBar";
import { ArrowLeft, ArrowUpRight, Hourglass, Scales, TrendDown, WarningCircle } from "@phosphor-icons/react";
import { useBasket, useVaultBasket } from "@/api/queries";
import { VaultPanel } from "@/features/vault/VaultPanel";
import { ApiError } from "@/api/client";
import { Badge, Card, ErrorState, SideBadge, Skeleton, Stat } from "@/components/ui/primitives";
import { formatPrice, formatUsdcCompact } from "@/lib/money";
import { formatDate, relative } from "@/lib/time";
import NotFound from "@/components/layout/NotFound";
import { BasketCover, tintFor } from "./BasketCard";
import { BasketBuyPanel } from "./BasketBuyPanel";

const LEG_COLORS = ["bg-accent", "bg-ink", "bg-ink/45", "bg-ink/20"];

export default function BasketDetailPage() {
  const { slug = "" } = useParams();
  const { data: b, isLoading, isError, error, refetch } = useBasket(slug);
  const desktop = useMediaQuery("(min-width: 1024px)");
  const vault = useVaultBasket(slug);
  const panelRef = useRef<HTMLDivElement>(null);

  if (isError && error instanceof ApiError && error.status === 404) return <NotFound />;
  if (isError) return <ErrorState error={error} onRetry={() => refetch()} />;
  if (isLoading || !b) {
    return (
      <div className="grid gap-8 lg:grid-cols-[1fr_400px]">
        <Skeleton className="h-80" />
        <Skeleton className="h-[520px]" />
      </div>
    );
  }

  const earliest = b.legs.map((l) => l.close_time).filter(Boolean).sort()[0] ?? null;

  return (
    <div>
      <Link to="/baskets" className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-ink">
        <ArrowLeft size={16} aria-hidden="true" /> Baskets
      </Link>

      <div className="mt-5 grid gap-8 lg:grid-cols-[1fr_400px] lg:gap-10">
        <div className="min-w-0 space-y-6">
          <section className="relative overflow-hidden rounded-[2rem] text-paper">
            <BasketCover legs={b.legs.length} tint={tintFor(b.slug)} className="absolute inset-0" />
            <div className="relative p-6 sm:p-9">
              <div className="flex flex-wrap items-center gap-2">
                <span className="rounded-full bg-paper/15 px-3 py-1.5 text-xs font-medium backdrop-blur">{b.category ?? "Basket"}</span>
                {b.status !== "published" && <Badge tone="warn">{b.status === "closed" ? "Closed for new buys" : b.status}</Badge>}
              </div>
              <h1 className="mt-16 max-w-[18ch] text-4xl font-semibold leading-[1.02] tracking-[-0.045em] sm:text-5xl">{b.title}</h1>
              <div className="mt-8 flex flex-wrap items-end gap-x-10 gap-y-4">
                <div>
                  <p className="text-xs text-paper/60">Index price</p>
                  <p className="font-display text-5xl font-semibold leading-none tracking-[-0.05em] num">{formatPrice(b.index_price).cents}</p>
                </div>
                <div className="text-sm text-paper/75 num">
                  <p>{b.legs.length} legs · first closes {relative(earliest)}</p>
                  {b.total_bought_usdc && <p>{formatUsdcCompact(b.total_bought_usdc)} bought through Tenor</p>}
                </div>
              </div>
            </div>
          </section>

          <Card className="p-6 sm:p-8">
            <p className="label-mono text-muted">The thesis</p>
            <p className="mt-3 text-lg leading-relaxed">{b.description}</p>
          </Card>

          {!desktop && (
            <div ref={panelRef} className="scroll-mt-24 space-y-4">
              <BasketBuyPanel basket={b} />
              {vault.data && <VaultPanel basket={b} vault={vault.data} />}
            </div>
          )}

          <Card className="p-5 sm:p-7">
            <div className="flex items-end justify-between gap-3">
              <h2 className="text-xl font-semibold tracking-tight">The legs</h2>
              <p className="text-xs text-muted">Index = weighted average of leg prices</p>
            </div>
            <div className="mt-5 flex h-2.5 gap-1 overflow-hidden rounded-full" role="img" aria-label={`Weights: ${b.legs.map((l) => `${l.market_title} ${l.weight_bps / 100}%`).join(", ")}`}>
              {b.legs.map((l, i) => (
                <span key={l.market_id} className={`${LEG_COLORS[i % 4]} rounded-full`} style={{ width: `${l.weight_bps / 100}%` }} />
              ))}
            </div>
            <ul className="mt-5 divide-y divide-line">
              {b.legs.map((l, i) => {
                const p = formatPrice(l.current_price);
                return (
                  <li key={l.market_id} className="py-4">
                    <div className="flex items-start gap-3">
                      <span className={`mt-1.5 size-2.5 shrink-0 rounded-full ${LEG_COLORS[i % 4]}`} aria-hidden="true" />
                      <div className="min-w-0 flex-1">
                        <Link to={`/markets/${encodeURIComponent(l.market_id)}`} className="group inline-flex items-start gap-1 font-medium leading-snug hover:underline underline-offset-4">
                          {l.market_title}
                          <ArrowUpRight size={14} className="mt-1 shrink-0 opacity-50 group-hover:opacity-100" aria-hidden="true" />
                        </Link>
                        <div className="mt-1.5 flex flex-wrap items-center gap-2 text-xs text-muted">
                          <SideBadge side={l.side} />
                          <span>Closes {formatDate(l.close_time)}</span>
                          {!l.tradable && <Badge tone="warn">Not tradable</Badge>}
                        </div>
                      </div>
                      <div className="text-right">
                        <p className="font-display text-lg font-semibold num">{p.cents}</p>
                        <p className="text-xs text-muted num">{l.weight_bps / 100}% weight</p>
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          </Card>

          <Card className="p-5 sm:p-7">
            <h2 className="text-xl font-semibold tracking-tight">Know the risks</h2>
            <ul className="mt-4 grid gap-3 sm:grid-cols-2">
              {[
                [Hourglass, "No selling early", "Panta markets are buy-only. Every leg is held until its market resolves, then you claim winners."],
                [TrendDown, "You can lose it all", "If every leg resolves against you, the basket is worth $0. Partly right still pays per leg."],
                [WarningCircle, "Prices move as you buy", "Markets use bonding curves, so bigger buys get worse prices. Check price impact in the quote."],
                [Scales, "Resolution is Panta's", "Tenor can't change or overturn a resolution. Check each market's rules and creator record."],
              ].map(([I, t, d]) => {
                const Icon = I as typeof Hourglass;
                return (
                  <li key={t as string} className="rounded-2xl bg-sunk p-4">
                    <Icon size={20} aria-hidden="true" />
                    <p className="mt-2 font-medium">{t as string}</p>
                    <p className="mt-0.5 text-sm text-muted">{d as string}</p>
                  </li>
                );
              })}
            </ul>
          </Card>

          <div className="grid grid-cols-2 gap-4 rounded-3xl border border-line bg-surface p-5 sm:grid-cols-3">
            <Stat label="Min buy" value={formatUsdcCompact(b.min_buy_usdc)} />
            <Stat label="Max buy" value={formatUsdcCompact(b.max_buy_usdc)} />
            <Stat label="Published" value={formatDate(b.published_at)} />
          </div>
        </div>

        {desktop && (
          <aside>
            <div className="sticky top-24 space-y-4">
              <BasketBuyPanel basket={b} />
              {vault.data && <VaultPanel basket={b} vault={vault.data} />}
            </div>
          </aside>
        )}
      </div>
      {!desktop && b.status === "published" && (
        <MobileActionBar target={panelRef}>
          <div className="flex items-center gap-3">
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-semibold">{b.title}</p>
              <p className="text-xs text-muted num">{b.legs.length} legs · index {formatPrice(b.index_price).cents}</p>
            </div>
            <button onClick={() => scrollToPanel(panelRef.current)} className="h-12 shrink-0 rounded-full bg-accent px-6 text-[15px] font-semibold text-ink">
              Buy basket
            </button>
          </div>
        </MobileActionBar>
      )}
    </div>
  );
}

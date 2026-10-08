import { useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { MagnifyingGlass, ChartLineUp } from "@phosphor-icons/react";
import { useCategories, useMarkets } from "@/api/queries";
import { Button } from "@/components/ui/Button";
import { EmptyState, ErrorState, PoweredByPanta } from "@/components/ui/primitives";
import { cn } from "@/lib/cn";
import { MarketCard, MarketCardSkeleton } from "./MarketCard";

const SORTS = [
  { value: "volume", label: "Top volume" },
  { value: "newest", label: "Newest" },
  { value: "closing_soon", label: "Closing soon" },
];

export default function MarketsPage() {
  const [params, setParams] = useSearchParams();
  const category = params.get("category");
  const sort = params.get("sort") ?? "volume";
  const q = params.get("q") ?? "";
  const [text, setText] = useState(q);

  // Debounced search into the URL so results are shareable/back-able.
  useEffect(() => {
    const t = setTimeout(() => {
      if (text === q) return;
      const next = new URLSearchParams(params);
      if (text) next.set("q", text);
      else next.delete("q");
      setParams(next, { replace: true });
    }, 300);
    return () => clearTimeout(t);
  }, [text, q, params, setParams]);

  const set = (k: string, v: string | null) => {
    const next = new URLSearchParams(params);
    if (v) next.set(k, v);
    else next.delete(k);
    setParams(next, { replace: true });
  };

  const cats = useCategories();
  const markets = useMarkets({ category, q: q || null, sort });
  const items = markets.data?.pages.flatMap((p) => p.items) ?? [];

  return (
    <div>
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="label-mono text-muted">Markets</p>
          <h1 className="mt-2 text-4xl font-semibold tracking-[-0.04em] sm:text-5xl">Every market, one place.</h1>
        </div>
        <PoweredByPanta />
      </div>

      <div className="sticky top-16 z-20 -mx-4 mt-8 space-y-3 bg-paper/90 px-4 py-3 backdrop-blur-md sm:-mx-6 sm:px-6">
        <div className="flex flex-col gap-3 sm:flex-row">
          <label className="relative flex-1">
            <span className="sr-only">Search markets</span>
            <MagnifyingGlass size={18} className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted" aria-hidden="true" />
            <input
              type="search"
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="Search markets: Osimhen, naira, SOL…"
              className="h-12 w-full rounded-full border border-line bg-surface pl-11 pr-4 text-base outline-none placeholder:text-faint focus:border-ink"
            />
          </label>
          <label className="relative">
            <span className="sr-only">Sort by</span>
            <select value={sort} onChange={(e) => set("sort", e.target.value)} className="h-12 w-full appearance-none rounded-full border border-line bg-surface px-5 pr-10 text-sm font-medium outline-none focus:border-ink sm:w-auto">
              {SORTS.map((s) => (
                <option key={s.value} value={s.value}>{s.label}</option>
              ))}
            </select>
            <span className="pointer-events-none absolute right-4 top-1/2 -translate-y-1/2 text-xs" aria-hidden="true">▾</span>
          </label>
        </div>
        <div className="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4 sm:mx-0 sm:px-0" role="group" aria-label="Category">
          {[{ id: "", name: "All" }, ...(cats.data ?? [])].map((c) => {
            const active = (category ?? "") === (c.id ? c.name : "");
            return (
              <button
                key={c.id || "all"}
                aria-pressed={active}
                onClick={() => set("category", c.id ? c.name : null)}
                className={cn("h-9 shrink-0 rounded-full border px-4 text-sm font-medium transition-colors", active ? "border-ink bg-ink text-paper" : "border-line bg-surface text-muted hover:text-ink")}
              >
                {c.name}
              </button>
            );
          })}
        </div>
      </div>

      <div className="mt-6">
        {markets.isError ? (
          <ErrorState error={markets.error} onRetry={() => markets.refetch()} />
        ) : markets.isLoading ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">{Array.from({ length: 6 }, (_, i) => <MarketCardSkeleton key={i} />)}</div>
        ) : items.length === 0 ? (
          <EmptyState icon={<ChartLineUp size={24} />} title="No markets match" body="Try a different search or category." action={<Button variant="secondary" onClick={() => { setText(""); setParams({}); }}>Clear filters</Button>} />
        ) : (
          <>
            <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {items.map((m) => (
                <li key={m.id}><MarketCard m={m} /></li>
              ))}
            </ul>
            {markets.hasNextPage && (
              <div className="mt-8 text-center">
                <Button variant="secondary" onClick={() => markets.fetchNextPage()} loading={markets.isFetchingNextPage}>Load more</Button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

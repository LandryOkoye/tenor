import { useMemo, useState } from "react";
import { Basket, CheckCircle, Path, Signature } from "@phosphor-icons/react";
import { useBaskets } from "@/api/queries";
import { EmptyState, ErrorState, PoweredByPanta } from "@/components/ui/primitives";
import { cn } from "@/lib/cn";
import { BasketCard, BasketCardSkeleton } from "./BasketCard";

export default function BasketsPage() {
  const { data, isLoading, isError, error, refetch } = useBaskets();
  const [cat, setCat] = useState<string | null>(null);
  const cats = useMemo(() => [...new Set((data ?? []).map((b) => b.category).filter(Boolean) as string[])], [data]);
  const list = (data ?? []).filter((b) => !cat || b.category === cat);

  return (
    <div>
      <section className="relative overflow-hidden rounded-[2rem] bg-ink px-6 py-10 text-paper sm:px-10 sm:py-14">
        <div className="relative z-10 max-w-xl">
          <p className="label-mono text-paper/50">Baskets</p>
          <h1 className="mt-3 text-4xl font-semibold leading-[1.02] tracking-[-0.045em] sm:text-6xl">
            Back a view, not a single bet<span className="text-accent">.</span>
          </h1>
          <p className="mt-4 text-paper/65">Curated bundles of 2–4 markets. One signature buys every leg, and each winning leg pays on its own.</p>
          <ul className="mt-8 grid gap-3 text-sm sm:grid-cols-3">
            {[
              [Path, "Curated legs + weights"],
              [Signature, "One signature"],
              [CheckCircle, "Paid per leg"],
            ].map(([I, t]) => {
              const Icon = I as typeof Path;
              return (
                <li key={t as string} className="flex items-center gap-2 rounded-xl bg-paper/[0.07] px-3 py-2.5">
                  <Icon size={18} className="text-accent" aria-hidden="true" /> {t as string}
                </li>
              );
            })}
          </ul>
        </div>
        <div className="pointer-events-none absolute -right-10 top-1/2 hidden h-[130%] w-[55%] -translate-y-1/2 md:block" aria-hidden="true">
          <svg viewBox="0 0 400 300" className="h-full w-full">
            {[40, 110, 190, 260].map((y, i) => (
              <path key={i} d={`M0 ${y} C 150 ${y}, 200 150, 300 150`} stroke="rgb(250 250 248 / 0.22)" strokeWidth="9" fill="none" strokeLinecap="round" />
            ))}
            <circle cx="324" cy="150" r="24" fill="#ff6b1a" className="origin-[324px_150px] animate-pulse-dot" />
          </svg>
        </div>
      </section>

      <div className="mt-8 flex flex-wrap items-center justify-between gap-4">
        <div className="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4 sm:mx-0 sm:px-0" role="group" aria-label="Theme">
          {[null, ...cats].map((c) => (
            <button
              key={c ?? "all"}
              aria-pressed={cat === c}
              onClick={() => setCat(c)}
              className={cn("h-9 shrink-0 rounded-full border px-4 text-sm font-medium", cat === c ? "border-ink bg-ink text-paper" : "border-line bg-surface text-muted hover:text-ink")}
            >
              {c ?? "All themes"}
            </button>
          ))}
        </div>
        <PoweredByPanta />
      </div>

      <div className="mt-6">
        {isError ? (
          <ErrorState error={error} onRetry={() => refetch()} />
        ) : isLoading ? (
          <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">{Array.from({ length: 3 }, (_, i) => <BasketCardSkeleton key={i} />)}</div>
        ) : list.length === 0 ? (
          <EmptyState icon={<Basket size={24} />} title="No baskets published yet" body="New baskets are curated from open markets. Check back soon, or browse single markets meanwhile." />
        ) : (
          <ul className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
            {list.map((b) => (
              <li key={b.id}><BasketCard b={b} /></li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

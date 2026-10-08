import { Link } from "react-router";
import { ArrowUpRight, Clock } from "@phosphor-icons/react";
import type { BasketTemplateSummary } from "@/api/types";
import { formatPrice, formatUsdcCompact } from "@/lib/money";
import { shortDuration } from "@/lib/time";
import { ConvergeArt } from "@/features/landing/visuals";

const TINTS = ["from-[#0e3b2c]/80", "from-[#3b230e]/80", "from-[#1c1d3b]/80", "from-[#3b0e2a]/70", "from-[#0e2a3b]/80"];

export function tintFor(key: string) {
  let h = 0;
  for (const c of key) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return TINTS[h % TINTS.length];
}

export function BasketCover({ legs, tint, className = "" }: { legs: number; tint: string; className?: string }) {
  return (
    <div className={`overflow-hidden bg-ink ${className}`} aria-hidden="true">
      <ConvergeArt legs={legs} className="absolute inset-0 h-full w-full" animated={false} />
      <div className={`absolute inset-0 bg-gradient-to-br ${tint} to-transparent`} />
    </div>
  );
}

export function BasketCard({ b }: { b: BasketTemplateSummary }) {
  const closes = b.closes_in_seconds == null ? null : Number(b.closes_in_seconds);
  return (
    <Link to={`/baskets/${b.slug}`} className="group flex h-full flex-col overflow-hidden rounded-[var(--radius-card)] border border-line bg-surface transition-[box-shadow,transform] duration-200 hover:-translate-y-0.5 hover:shadow-soft">
      <div className="relative h-44 text-paper">
        <BasketCover legs={b.leg_count} tint={tintFor(b.slug)} className="absolute inset-0 transition-transform duration-700 group-hover:scale-[1.03]" />
        <span className="absolute left-4 top-4 rounded-full bg-paper/15 px-3 py-1.5 text-xs font-medium backdrop-blur">{b.category ?? "Basket"}</span>
        <span className="absolute right-4 top-4 grid size-9 place-items-center rounded-full bg-paper/15 backdrop-blur transition-transform group-hover:-translate-y-0.5 group-hover:translate-x-0.5">
          <ArrowUpRight size={16} aria-hidden="true" />
        </span>
        <div className="absolute bottom-4 left-4">
          <p className="text-[11px] text-paper/60">Index price</p>
          <p className="font-display text-4xl font-semibold leading-none tracking-[-0.04em] num">{formatPrice(b.index_price).cents}</p>
        </div>
      </div>
      <div className="flex flex-1 flex-col p-5">
        <h3 className="text-lg font-semibold leading-snug tracking-tight">{b.title}</h3>
        <p className="mt-1.5 line-clamp-2 flex-1 text-sm text-muted">{b.description}</p>
        <div className="mt-4 flex items-center gap-3 border-t border-line pt-4 text-xs text-muted num">
          <span className="font-medium text-ink">{b.leg_count} legs</span>
          <span aria-hidden="true">·</span>
          <span className="inline-flex items-center gap-1"><Clock size={13} aria-hidden="true" />{closes != null ? `Closes in ${shortDuration(closes)}` : "Open"}</span>
          {b.total_bought_usdc && <span className="ml-auto">{formatUsdcCompact(b.total_bought_usdc)} bought</span>}
        </div>
      </div>
    </Link>
  );
}

export function BasketCardSkeleton() {
  return (
    <div className="overflow-hidden rounded-[var(--radius-card)] border border-line bg-surface">
      <div className="h-44 animate-pulse bg-sunk" />
      <div className="space-y-2 p-5">
        <div className="h-5 w-2/3 animate-pulse rounded bg-sunk" />
        <div className="h-4 w-full animate-pulse rounded bg-sunk" />
        <div className="h-4 w-4/5 animate-pulse rounded bg-sunk" />
      </div>
    </div>
  );
}

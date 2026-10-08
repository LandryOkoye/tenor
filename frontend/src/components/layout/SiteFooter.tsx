import { Link } from "react-router";
import { Mark } from "@/components/brand/Logo";
import { PoweredByPanta } from "@/components/ui/primitives";
import { cn } from "@/lib/cn";

const COLS = [
  { title: "Trade", links: [["Markets", "/markets"], ["Baskets", "/baskets"], ["Portfolio", "/portfolio"]] },
  { title: "Trust", links: [["Resolution Center", "/resolution"], ["How fees work", "/learn#fees"], ["Risk disclosure", "/legal#risk"]] },
  { title: "Learn", links: [["How markets work", "/learn#markets"], ["Get a wallet", "/learn#wallet"], ["Get USDC on Solana", "/learn#usdc"]] },
];

/** Charter-style footer with the giant wordmark; `big` adds the wordmark (landing). */
export function SiteFooter({ big = false }: { big?: boolean }) {
  return (
    <footer className={cn("border-t border-line bg-paper", big ? "pt-20" : "pt-14")}>
      <div className="mx-auto grid max-w-7xl grid-cols-2 gap-x-6 gap-y-10 px-4 sm:grid-cols-3 sm:px-6 md:grid-cols-[1.4fr_repeat(3,1fr)]">
        <div className="col-span-2 sm:col-span-3 md:col-span-1">
          <Link to="/" className="inline-flex items-center gap-2 text-2xl font-display font-bold tracking-[-0.06em]">
            <Mark className="h-6" /> tenor
          </Link>
          <p className="mt-4 max-w-xs text-sm text-muted">Prediction markets that feel like investing, not gambling. Non-custodial, settled in USDC on Solana, open worldwide.</p>
          <PoweredByPanta className="mt-5" />
        </div>
        {COLS.map((c) => (
          <div key={c.title}>
            <p className="text-sm font-medium">{c.title}</p>
            <ul className="mt-4 space-y-2.5">
              {c.links.map(([l, to]) => (
                <li key={l}>
                  <Link to={to} className="text-sm text-muted hover:text-ink">{l}</Link>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
      {big && (
        <div className="mx-auto max-w-7xl overflow-hidden px-4 sm:px-6" aria-hidden="true">
          <p className="display-xl -mb-[0.06em] select-none pt-10 text-center text-[39vw] leading-[0.9] text-ink/90 xl:text-[520px]">tenor</p>
        </div>
      )}
      <div className="mx-auto mt-10 flex max-w-7xl flex-col gap-2 border-t border-line px-4 py-6 pb-24 text-xs text-muted sm:px-6 md:flex-row md:justify-between md:pb-6">
        <p>© {new Date().getFullYear()} Tenor. 18+ only. Prediction markets carry risk of loss. Not available in restricted countries.</p>
        <p>Markets, liquidity and settlement by Panta. Tenor never holds your funds.</p>
      </div>
    </footer>
  );
}

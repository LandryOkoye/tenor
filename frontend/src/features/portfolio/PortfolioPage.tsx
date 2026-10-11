import { useState } from "react";
import { Link, useSearchParams } from "react-router";
import { ArrowUpRight, Basket, ClockCounterClockwise, Trophy, Wallet, WarningCircle } from "@phosphor-icons/react";
import type { ActivityItem, LegStatus, PortfolioBasket, PortfolioPosition } from "@/api/types";
import { useActivity, usePortfolio } from "@/api/queries";
import { friendlyError } from "@/api/client";
import { Badge, Card, EmptyState, ErrorState, SideBadge, Skeleton, Tabs } from "@/components/ui/primitives";
import { Button, ButtonLink } from "@/components/ui/Button";
import { cn } from "@/lib/cn";
import { formatPrice, formatShares, formatUsdc, fromBaseUnits } from "@/lib/money";
import { formatDate } from "@/lib/time";
import { useTenorWallet } from "@/wallet/useTenorWallet";
import { useSession } from "@/wallet/session";
import { ClaimButton } from "./ClaimButton";

type Tab = "baskets" | "positions" | "resolved" | "activity";

export default function PortfolioPage() {
  const wallet = useTenorWallet();
  const { session, ensureSession, signingIn } = useSession();
  const [err, setErr] = useState<string | null>(null);
  const [params, setParams] = useSearchParams();
  const tab = (params.get("tab") as Tab) ?? "baskets";
  const portfolio = usePortfolio();

  if (!wallet.address) {
    return (
      <Gate icon={<Wallet size={24} />} title="Connect your wallet to see your portfolio" body="Your positions live in your own Solana wallet. Connect it to see baskets, P&L and anything you can claim.">
        <Button onClick={wallet.openModal}>Connect wallet</Button>
      </Gate>
    );
  }
  if (!session) {
    return (
      <Gate icon={<Wallet size={24} />} title="Sign in to load your portfolio" body="Sign a free message with your wallet. It proves the wallet is yours and doesn't move any funds.">
        <Button loading={signingIn} onClick={async () => { setErr(null); try { await ensureSession(); } catch (e) { setErr(friendlyError(e)); } }}>Sign in with wallet</Button>
        {err && <p role="alert" className="mt-3 text-sm text-no">{err}</p>}
      </Gate>
    );
  }
  if (portfolio.isError) return <ErrorState error={portfolio.error} onRetry={() => portfolio.refetch()} />;
  if (portfolio.isLoading || !portfolio.data) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-44" />
        <Skeleton className="h-12 w-80" />
        <Skeleton className="h-64" />
      </div>
    );
  }

  const p = portfolio.data;
  const open = p.single_positions.filter((x) => !x.resolved);
  const resolved = p.single_positions.filter((x) => x.resolved);
  const claimable = [
    ...new Set([...p.single_positions.filter((x) => x.claimable).map((x) => x.market_id), ...p.baskets.flatMap((b) => b.legs.filter((l) => l.claimable).map((l) => l.market_id))]),
  ];
  const claimableValue =
    p.single_positions.filter((x) => x.claimable).reduce((s, x) => s + fromBaseUnits(x.value_usdc), 0) +
    p.baskets.flatMap((b) => b.legs).filter((l) => l.claimable).reduce((s, l) => s + fromBaseUnits(l.value_usdc), 0);
  const pnl = fromBaseUnits(p.total_unrealized_pnl_usdc);
  const cost = fromBaseUnits(p.total_cost_basis_usdc);

  return (
    <div>
      <section className="grid gap-4 lg:grid-cols-[1.4fr_1fr]">
        <div className="relative overflow-hidden rounded-[2rem] bg-ink p-6 text-paper sm:p-8">
          <p className="label-mono text-paper/50">Portfolio value</p>
          <p className="mt-3 font-display text-5xl font-semibold tracking-[-0.045em] num sm:text-6xl">{formatUsdc(p.total_value_usdc)}</p>
          <p className={cn("mt-2 text-sm font-medium num", pnl >= 0 ? "text-[#5fd6a4]" : "text-[#ff8a7a]")}>
            {formatUsdc(p.total_unrealized_pnl_usdc, { sign: true })} {cost > 0 && `· ${pnl >= 0 ? "+" : ""}${((pnl / cost) * 100).toFixed(1)}%`} unrealised
          </p>
          <div className="mt-8 grid grid-cols-3 gap-4 border-t border-paper/10 pt-5 text-sm">
            <div><p className="text-paper/50">Cost basis</p><p className="mt-1 font-medium num">{formatUsdc(p.total_cost_basis_usdc)}</p></div>
            <div><p className="text-paper/50">Baskets</p><p className="mt-1 font-medium num">{p.baskets.length}</p></div>
            <div><p className="text-paper/50">Positions</p><p className="mt-1 font-medium num">{open.length}</p></div>
          </div>
          <div className="pointer-events-none absolute -right-16 -top-16 size-56 rounded-full bg-accent/30 blur-3xl" aria-hidden="true" />
        </div>
        <div className={cn("flex flex-col justify-between rounded-[2rem] border p-6 sm:p-8", claimable.length ? "border-accent/40 bg-accent-soft" : "border-line bg-surface")}>
          <div>
            <p className="flex items-center gap-2 font-medium"><Trophy size={20} weight={claimable.length ? "fill" : "regular"} className={claimable.length ? "text-accent-ink" : ""} aria-hidden="true" /> Winnings to claim</p>
            <p className="mt-3 font-display text-4xl font-semibold tracking-tight num">${claimableValue.toFixed(2)}</p>
            <p className="mt-1 text-sm text-muted">{claimable.length ? `${claimable.length} winning market${claimable.length > 1 ? "s" : ""}, about $1.00 per share` : "Nothing to claim yet. Wins show up here when markets resolve."}</p>
          </div>
          {claimable.length > 0 && <ClaimButton marketIds={claimable} label="Claim all" variant="primary" size="lg" className="mt-6 w-full" />}
        </div>
      </section>

      <Tabs<Tab>
        label="Portfolio sections"
        className="mt-8 w-full sm:w-fit"
        value={tab}
        onChange={(t) => setParams({ tab: t }, { replace: true })}
        items={[
          { value: "baskets", label: <>Baskets <span className="hidden opacity-60 num sm:inline">{p.baskets.length}</span></> },
          { value: "positions", label: <>Positions <span className="hidden opacity-60 num sm:inline">{open.length}</span></> },
          { value: "resolved", label: <>Resolved <span className="hidden opacity-60 num sm:inline">{resolved.length}</span></> },
          { value: "activity", label: "Activity" },
        ]}
      />

      <div className="mt-6" role="tabpanel">
        {tab === "baskets" &&
          (p.baskets.length === 0 ? (
            <EmptyState icon={<Basket size={24} />} title="No baskets yet" body="Buy a whole view in one signature." action={<ButtonLink to="/baskets">Explore baskets</ButtonLink>} />
          ) : (
            <div className="space-y-4">{p.baskets.map((b) => <BasketRow key={b.purchase_id} b={b} />)}</div>
          ))}
        {tab === "positions" &&
          (open.length === 0 ? (
            <EmptyState icon={<Wallet size={24} />} title="No open single positions" action={<ButtonLink to="/markets">Browse markets</ButtonLink>} />
          ) : (
            <PositionsTable rows={open} />
          ))}
        {tab === "resolved" &&
          (resolved.length === 0 ? <EmptyState icon={<Trophy size={24} />} title="Nothing resolved yet" body="Single positions show here once their markets resolve." /> : <PositionsTable rows={resolved} />)}
        {tab === "activity" && <Activity />}
      </div>
    </div>
  );
}

function Gate({ icon, title, body, children }: { icon: React.ReactNode; title: string; body: string; children: React.ReactNode }) {
  return (
    <div className="mx-auto max-w-lg py-16">
      <EmptyState icon={icon} title={title} body={body} action={<div className="flex flex-col items-center">{children}</div>} />
    </div>
  );
}

const LEG_LABEL: Record<LegStatus, [string, "neutral" | "yes" | "no" | "accent" | "warn" | "dark"]> = {
  pending: ["Pending", "warn"],
  filled: ["Holding", "neutral"],
  failed: ["Failed", "no"],
  resolved_win: ["Won", "yes"],
  resolved_loss: ["Lost", "neutral"],
  claimed: ["Claimed", "dark"],
};

function BasketRow({ b }: { b: PortfolioBasket }) {
  const pnl = fromBaseUnits(b.unrealized_pnl_usdc);
  const claimable = b.legs.filter((l) => l.claimable).map((l) => l.market_id);
  return (
    <Card as="article" className="p-5 sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <Link to={`/baskets/${b.template_slug}`} className="group inline-flex items-center gap-1 text-lg font-semibold tracking-tight hover:underline underline-offset-4">
            {b.template_title} <ArrowUpRight size={16} className="opacity-50 group-hover:opacity-100" aria-hidden="true" />
          </Link>
          <p className="mt-1 text-sm text-muted num">{b.legs_resolved} of {b.legs_total} legs resolved</p>
        </div>
        <div className="sm:text-right">
          <p className="font-display text-2xl font-semibold tracking-tight num">{formatUsdc(b.current_value_usdc)}</p>
          <p className={cn("text-sm num", pnl >= 0 ? "text-yes" : "text-no")}>{formatUsdc(b.unrealized_pnl_usdc, { sign: true })} on {formatUsdc(b.cost_basis_usdc)}</p>
        </div>
      </div>
      {b.modified_outside && (
        <p className="mt-4 flex gap-2 rounded-xl bg-warn-soft px-3 py-2.5 text-sm">
          <WarningCircle size={18} className="shrink-0 text-warn" aria-hidden="true" /> Basket modified outside Tenor: your wallet's shares no longer match this purchase. Values follow your wallet.
        </p>
      )}
      <ul className="mt-4 divide-y divide-line rounded-2xl border border-line">
        {b.legs.map((l) => {
          const [label, tone] = LEG_LABEL[l.status];
          return (
            <li key={l.order_id} className="grid grid-cols-[1fr_auto] items-center gap-x-3 gap-y-2 px-4 py-3 sm:flex sm:flex-wrap">
              <Link to={`/markets/${encodeURIComponent(l.market_id)}`} className="col-span-2 text-sm font-medium leading-snug hover:underline underline-offset-4 sm:order-2 sm:min-w-0 sm:flex-1">{l.market_title}</Link>
              <span className="flex items-center gap-2 sm:order-1">
                <SideBadge side={l.side} />
                <span className="sm:hidden"><Badge tone={tone}>{label}</Badge></span>
              </span>
              <span className="hidden sm:order-3 sm:inline-flex"><Badge tone={tone}>{label}</Badge></span>
              <span className="text-right text-sm num sm:order-4 sm:w-24">
                {formatUsdc(l.value_usdc)}
                <span className="block text-xs text-muted">{formatShares(l.shares)} sh</span>
              </span>
              {l.claimable && (
                <span className="col-span-2 sm:order-5 sm:col-span-1 [&>button]:w-full sm:[&>button]:w-auto">
                  <ClaimButton marketIds={[l.market_id]} label="Claim" size="sm" variant="accent" />
                </span>
              )}
            </li>
          );
        })}
      </ul>
      {claimable.length > 1 && (
        <div className="mt-4 flex justify-end">
          <ClaimButton marketIds={claimable} label={`Claim ${claimable.length} legs`} size="sm" />
        </div>
      )}
    </Card>
  );
}

function PositionsTable({ rows }: { rows: PortfolioPosition[] }) {
  return (
    <Card className="overflow-hidden">
      <ul className="divide-y divide-line">
        {rows.map((r) => {
          const pnl = r.unrealized_pnl_usdc ? fromBaseUnits(r.unrealized_pnl_usdc) : null;
          return (
            <li key={`${r.market_id}-${r.side}`} className="flex flex-wrap items-center gap-x-3 gap-y-2 px-5 py-4">
              <SideBadge side={r.side} />
              <div className="min-w-0 flex-1 basis-[calc(100%-4rem)] sm:basis-auto">
                <Link to={`/markets/${encodeURIComponent(r.market_id)}`} className="font-medium hover:underline underline-offset-4">{r.market_title}</Link>
                <p className="text-xs text-muted num">
                  {formatShares(r.shares)} shares {r.resolved ? (r.won ? "· resolved your way" : "· resolved against you") : `· now ${formatPrice(r.current_price).cents}`}
                </p>
              </div>
              {r.resolved && <Badge tone={r.won ? "yes" : "neutral"}>{r.won ? "Won" : "Lost"}</Badge>}
              <div className="ml-auto w-28 text-right num">
                <p className="font-medium">{formatUsdc(r.value_usdc)}</p>
                {pnl != null && <p className={cn("text-xs", pnl >= 0 ? "text-yes" : "text-no")}>{formatUsdc(r.unrealized_pnl_usdc, { sign: true })}</p>}
              </div>
              {r.claimable && <ClaimButton marketIds={[r.market_id]} label="Claim" size="sm" variant="accent" />}
            </li>
          );
        })}
      </ul>
    </Card>
  );
}

const KIND: Record<ActivityItem["kind"], string> = { buy: "Bought", basket_buy: "Bought basket", claim: "Claimed" };

function Activity() {
  const { data, isLoading, isError, error, refetch } = useActivity();
  if (isLoading) return <Skeleton className="h-64" />;
  if (isError) return <ErrorState error={error} onRetry={() => refetch()} />;
  if (!data?.length) return <EmptyState icon={<ClockCounterClockwise size={24} />} title="No activity yet" />;
  return (
    <Card className="overflow-hidden">
      <ul className="divide-y divide-line">
        {data.map((a, i) => (
          <li key={`${a.tx_signature}-${i}`} className="flex flex-wrap items-center gap-3 px-5 py-4">
            <span className={cn("grid size-9 place-items-center rounded-full", a.kind === "claim" ? "bg-yes-soft text-yes" : "bg-sunk")}>
              {a.kind === "claim" ? <Trophy size={16} aria-hidden="true" /> : a.kind === "basket_buy" ? <Basket size={16} aria-hidden="true" /> : <Wallet size={16} aria-hidden="true" />}
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">{KIND[a.kind]} {a.side && <SideBadge side={a.side} className="ml-1" />}</p>
              <p className="truncate text-sm text-muted">{a.market_title ?? "–"}</p>
            </div>
            <div className="text-right text-sm num">
              <p className={cn("font-medium", a.kind === "claim" && "text-yes")}>{a.kind === "claim" ? "+" : "−"}{formatUsdc(a.amount_usdc)}</p>
              <p className="text-xs text-muted">{formatDate(a.ts, true)}</p>
            </div>
            {a.tx_signature && (
              <a href={`https://solscan.io/tx/${a.tx_signature}`} target="_blank" rel="noreferrer" className="grid size-9 place-items-center rounded-full text-muted hover:bg-sunk hover:text-ink" aria-label="View transaction on Solscan">
                <ArrowUpRight size={16} aria-hidden="true" />
              </a>
            )}
          </li>
        ))}
      </ul>
    </Card>
  );
}

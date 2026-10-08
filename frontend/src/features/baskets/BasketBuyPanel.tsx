import { useCallback, useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";
import { CheckCircle, CircleNotch, GearSix, Hourglass, XCircle } from "@phosphor-icons/react";
import type { BasketPurchaseView, BasketQuote, BasketTemplateDetail, LegStatus } from "@/api/types";
import { api } from "@/api/endpoints";
import { ApiError, friendlyError } from "@/api/client";
import { qk } from "@/api/queries";
import { Button } from "@/components/ui/Button";
import { PoweredByPanta, SideBadge } from "@/components/ui/primitives";
import { cn } from "@/lib/cn";
import { formatBps, formatPrice, formatShares, formatUsdc, fromBaseUnits, toBaseUnits } from "@/lib/money";
import { useSession } from "@/wallet/session";
import { useTenorWallet } from "@/wallet/useTenorWallet";
import { AmountInput } from "@/features/trade/AmountInput";
import { QuoteRow } from "@/features/trade/QuoteRows";
import { SignReview, useCountdown } from "@/features/trade/SignReview";
import { useSignFlow } from "@/features/trade/useSignFlow";

const SLIPPAGES = [100, 150, 200, 300];
const TERMINAL = ["filled", "partial", "failed", "active", "partially_resolved", "resolved", "claimed"];

export function BasketBuyPanel({ basket }: { basket: BasketTemplateDetail }) {
  const wallet = useTenorWallet();
  const { ensureSession } = useSession();
  const qc = useQueryClient();
  const min = fromBaseUnits(basket.min_buy_usdc);
  const max = fromBaseUnits(basket.max_buy_usdc);
  const [amount, setAmount] = useState(String(Math.max(10, Math.ceil(min))));
  const [slippage, setSlippage] = useState(150);
  const [showSettings, setShowSettings] = useState(false);
  const [quote, setQuote] = useState<BasketQuote | null>(null);
  const [quoting, setQuoting] = useState(false);
  const [quoteErr, setQuoteErr] = useState<string | null>(null);
  const [requoted, setRequoted] = useState(false);

  const base = toBaseUnits(amount);
  const n = base ? fromBaseUnits(base) : 0;
  const amountErr = !amount ? null : !base ? "Enter a valid amount" : n < min ? `Minimum is $${min}` : n > max ? `Maximum is $${max}` : null;
  const tradable = basket.status === "published" && basket.legs.every((l) => l.tradable);

  useEffect(() => {
    setQuote(null);
    setRequoted(false);
  }, [amount, slippage]);

  const getQuote = useCallback(async () => {
    if (!base || amountErr) return;
    setQuoteErr(null);
    setQuoting(true);
    try {
      await ensureSession();
      setQuote(await api.basketQuote(basket.id, { amount_usdc: base, slippage_bps: slippage }));
    } catch (e) {
      setQuoteErr(friendlyError(e));
    } finally {
      setQuoting(false);
    }
  }, [amountErr, base, basket.id, ensureSession, slippage]);

  const flow = useSignFlow<BasketPurchaseView>({
    build: async () => {
      try {
        const b = await api.basketBuild(quote!.quote_id);
        return { id: b.purchase_id, transactions: b.transactions, summary: b.summary, expires_at: b.expires_at };
      } catch (e) {
        if (e instanceof ApiError && (e.code === "QUOTE_STALE" || e.code === "SLIPPAGE_EXCEEDED") && !requoted) {
          await getQuote();
          setRequoted(true);
          throw new ApiError(e.code, "Prices moved, so we requoted the whole basket. Check the new totals and tap Buy again.", 409);
        }
        throw e;
      }
    },
    submit: (id, signed) => api.basketSubmit(id, { signed_txs: signed }),
  });

  const purchaseId = flow.result?.id;
  const purchase = useQuery({
    queryKey: qk.purchase(purchaseId ?? ""),
    queryFn: () => api.purchase(purchaseId!),
    enabled: !!purchaseId,
    initialData: flow.result ?? undefined,
    refetchInterval: (q) => (q.state.data && TERMINAL.includes(q.state.data.status) ? false : 1500),
  });
  useEffect(() => {
    if (purchase.data && TERMINAL.includes(purchase.data.status)) {
      qc.invalidateQueries({ queryKey: ["portfolio"] });
      qc.invalidateQueries({ queryKey: qk.basket(basket.slug) });
    }
  }, [purchase.data, qc, basket.slug]);

  const left = useCountdown(quote?.expires_at);
  const expired = left !== null && left <= 0;

  if (flow.phase === "done" && purchase.data) {
    return <PurchaseProgress p={purchase.data} onDone={() => { flow.reset(); setQuote(null); }} />;
  }

  return (
    <div className="rounded-[var(--radius-card)] border border-line bg-surface p-5 sm:p-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold tracking-tight">Buy this basket</h2>
        <button onClick={() => setShowSettings((s) => !s)} aria-expanded={showSettings} className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1.5 text-xs font-medium text-muted hover:bg-sunk hover:text-ink">
          <GearSix size={15} aria-hidden="true" /> Slippage {formatBps(slippage)}
        </button>
      </div>
      {showSettings && (
        <div className="mt-3 rounded-2xl bg-sunk p-3">
          <p className="text-xs text-muted">If the total moves more than this before you sign, we requote and ask you again.</p>
          <div className="mt-2 grid grid-cols-4 gap-1.5" role="radiogroup" aria-label="Slippage tolerance">
            {SLIPPAGES.map((s) => (
              <button key={s} role="radio" aria-checked={slippage === s} onClick={() => setSlippage(s)} className={cn("h-9 rounded-full text-sm font-medium num", slippage === s ? "bg-ink text-paper" : "bg-surface")}>
                {formatBps(s)}
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="mt-4">
        <AmountInput value={amount} onChange={setAmount} min={min} max={max} error={amountErr} />
      </div>

      {/* Per-leg allocation: live quote when we have one, else weights preview. */}
      <div className="mt-5">
        <p className="label-mono text-muted">{quote && !expired ? "Your quote" : "How it splits"}</p>
        <ul className="mt-2 space-y-2">
          {(quote && !expired ? quote.legs : basket.legs).map((l) => {
            const q = quote && !expired ? (l as BasketQuote["legs"][number]) : null;
            const alloc = q ? fromBaseUnits(q.allocation_usdc) : (n * l.weight_bps) / 10_000;
            return (
              <li key={l.market_id} className="rounded-2xl border border-line px-3.5 py-3">
                <div className="flex items-start gap-2.5">
                  <SideBadge side={l.side} className="mt-0.5" />
                  <span className="flex-1 text-sm font-medium leading-snug">{l.market_title}</span>
                  <span className="text-sm font-semibold num">${alloc.toFixed(2)}</span>
                </div>
                {q && (
                  <p className="mt-1.5 pl-[46px] text-xs text-muted num">
                    ~{formatShares(q.expected_shares)} shares @ {formatPrice(q.avg_price).cents} · impact {formatBps(q.price_impact_bps)}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      </div>

      {quote && !expired && (
        <dl className="mt-4 rounded-2xl bg-sunk px-4 py-3">
          <QuoteRow label="Tenor fee" value={formatUsdc(quote.our_fee_usdc)} />
          <QuoteRow label="Panta fees (est.)" value={formatUsdc(quote.total_panta_fees_usdc)} />
          <QuoteRow label="Max payout if every leg wins" value={`$${quote.legs.reduce((s, l) => s + Number(l.expected_shares), 0).toFixed(2)}`} />
          <div className="mt-1 border-t border-line pt-1">
            <QuoteRow strong label="Total" value={formatUsdc(quote.amount_usdc)} />
          </div>
          <p className="mt-1 text-right text-xs text-muted num">Quote valid {Math.max(0, Math.ceil(left ?? 0))}s</p>
        </dl>
      )}

      {(quoteErr || flow.error) && <p role="alert" className="mt-4 rounded-xl bg-no-soft px-3 py-2.5 text-sm text-no">{quoteErr ?? flow.error}</p>}

      <div className="mt-5">
        {!tradable ? (
          <Button size="lg" className="w-full" disabled>Closed for new purchases</Button>
        ) : !wallet.address ? (
          <Button size="lg" className="w-full" onClick={wallet.openModal}>Connect wallet to buy</Button>
        ) : !quote || expired ? (
          <Button size="lg" className="w-full" onClick={getQuote} loading={quoting} disabled={!base || !!amountErr}>
            {expired ? "Refresh quote" : "Get quote"}
          </Button>
        ) : (
          <Button size="lg" variant="accent" className="w-full" onClick={flow.start} loading={flow.phase === "building"}>
            Buy basket · {formatUsdc(quote.amount_usdc)}
          </Button>
        )}
      </div>
      <div className="mt-4 flex items-start justify-between gap-3 text-xs text-muted">
        <p className="flex gap-1.5"><Hourglass size={14} className="mt-px shrink-0" aria-hidden="true" /> Every leg is held until its market resolves.</p>
        <PoweredByPanta className="shrink-0" />
      </div>

      <SignReview
        open={flow.phase === "review" || flow.phase === "signing"}
        onClose={flow.cancel}
        onSign={flow.sign}
        summary={flow.built?.summary ?? null}
        expiresAt={flow.built?.expires_at ?? null}
        txCount={flow.built?.transactions.length ?? 1}
        signing={flow.phase === "signing"}
        title="Review basket"
        error={flow.error}
      />
    </div>
  );
}

const LEG_ICON: Record<LegStatus, React.ReactNode> = {
  pending: <CircleNotch size={20} className="animate-spin text-muted" aria-label="Pending" />,
  filled: <CheckCircle size={20} weight="fill" className="text-yes" aria-label="Filled" />,
  failed: <XCircle size={20} weight="fill" className="text-no" aria-label="Failed" />,
  resolved_win: <CheckCircle size={20} weight="fill" className="text-yes" aria-label="Won" />,
  resolved_loss: <XCircle size={20} weight="fill" className="text-muted" aria-label="Lost" />,
  claimed: <CheckCircle size={20} weight="fill" className="text-ink" aria-label="Claimed" />,
};

/** Per-leg progress after signing (brief §12, §7.5). */
export function PurchaseProgress({ p, onDone }: { p: BasketPurchaseView; onDone: () => void }) {
  const filled = p.legs.filter((l) => l.status === "filled").length;
  const done = TERMINAL.includes(p.status);
  const partial = p.status === "partial";
  const failed = p.status === "failed";
  return (
    <div className="rounded-[var(--radius-card)] border border-line bg-surface p-5 sm:p-6" aria-live="polite">
      <p className="label-mono text-muted">{done ? (failed ? "Purchase failed" : partial ? "Partly filled" : "Purchase complete") : "Confirming on Solana"}</p>
      <h2 className="mt-2 text-2xl font-semibold tracking-tight">
        {failed ? "Nothing was bought." : partial ? `${filled} of ${p.legs.length} legs filled` : done ? "Your basket is live." : "Buying your legs…"}
      </h2>
      <div className="mt-4 h-1.5 overflow-hidden rounded-full bg-sunk">
        <div className={cn("h-full rounded-full transition-[width] duration-500", failed ? "bg-no" : "bg-accent")} style={{ width: `${(filled / p.legs.length) * 100}%` }} />
      </div>
      <ul className="mt-5 space-y-2">
        {p.legs.map((l) => (
          <li key={l.order_id} className="flex items-center gap-3 rounded-2xl border border-line px-3.5 py-3">
            {LEG_ICON[l.status]}
            <span className="flex-1 text-sm font-medium leading-snug">{l.market_title}</span>
            <span className="text-right text-xs text-muted num">
              {l.shares ? `${formatShares(l.shares)} sh` : formatUsdc(l.amount_usdc)}
              {l.error_code && <span className="block text-no">{l.error_code}</span>}
            </span>
          </li>
        ))}
      </ul>
      {failed && <p className="mt-4 text-sm text-muted">The transaction didn't go through, so your USDC is untouched. Get a new quote to try again.</p>}
      {partial && <p className="mt-4 text-sm text-muted">Some parts failed because prices moved. You keep the filled legs. Retry to buy the missing ones at current prices.</p>}
      <div className="mt-6 flex gap-2">
        <Button variant="secondary" className="flex-1" onClick={onDone}>{failed || partial ? "Try again" : "Buy more"}</Button>
        <Link to="/portfolio" className="inline-flex h-11 flex-1 items-center justify-center rounded-full bg-ink text-[15px] font-medium text-paper">Portfolio</Link>
      </div>
    </div>
  );
}

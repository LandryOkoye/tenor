import { useCallback, useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle, Hourglass, Info, XCircle } from "@phosphor-icons/react";
import { Link } from "react-router";
import type { MarketDetail, OrderQuote, OrderView, Side } from "@/api/types";
import { api } from "@/api/endpoints";
import { ApiError, friendlyError } from "@/api/client";
import { Button, Spinner } from "@/components/ui/Button";
import { PoweredByPanta } from "@/components/ui/primitives";
import { cn } from "@/lib/cn";
import { formatBps, formatPrice, formatShares, formatUsdc, sharesToNumber, toBaseUnits } from "@/lib/money";
import { useSession } from "@/wallet/session";
import { useTenorWallet } from "@/wallet/useTenorWallet";
import { AmountInput } from "@/features/trade/AmountInput";
import { QuoteRow } from "@/features/trade/QuoteRows";
import { SignReview, useCountdown } from "@/features/trade/SignReview";
import { useSignFlow } from "@/features/trade/useSignFlow";

/** Single-market YES/NO buy (Panta primary buy via our API). */
export function BuyPanel({ market, initialSide = "yes", requestedSide }: { market: MarketDetail; initialSide?: Side; requestedSide?: { side: Side; n: number } }) {
  const wallet = useTenorWallet();
  const { ensureSession } = useSession();
  const qc = useQueryClient();
  const [side, setSide] = useState<Side>(initialSide);
  const [amount, setAmount] = useState("10");
  const [quote, setQuote] = useState<OrderQuote | null>(null);
  const [quoting, setQuoting] = useState(false);
  const [quoteErr, setQuoteErr] = useState<string | null>(null);
  const [requoted, setRequoted] = useState(false);
  const tradable = market.status === "open" && market.yes_price != null;

  useEffect(() => {
    if (requestedSide) setSide(requestedSide.side);
  }, [requestedSide]);

  const amountBase = toBaseUnits(amount);
  const amountN = amountBase ? Number(amount) : 0;
  const amountErr = !amount ? null : !amountBase ? "Enter a valid amount" : amountN < 0.01 || amountN > 1000 ? "Single buys are $0.01 to $1,000" : null;

  // Inputs changed → old quote no longer applies.
  useEffect(() => {
    setQuote(null);
    setRequoted(false);
  }, [side, amount]);

  const getQuote = useCallback(async () => {
    if (!amountBase) return null;
    setQuoteErr(null);
    setQuoting(true);
    try {
      await ensureSession();
      const q = await api.orderQuote({ market_id: market.id, side, amount_usdc: amountBase });
      setQuote(q);
      return q;
    } catch (e) {
      setQuoteErr(friendlyError(e));
      return null;
    } finally {
      setQuoting(false);
    }
  }, [amountBase, ensureSession, market.id, side]);

  const flow = useSignFlow<OrderView>({
    build: async () => {
      try {
        const b = await api.orderBuild(quote!.quote_id);
        return { id: b.order_id, transactions: b.transactions, summary: b.summary, expires_at: b.expires_at };
      } catch (e) {
        // QUOTE_STALE: requote once and let the user confirm the new numbers (brief §7.3).
        if (e instanceof ApiError && (e.code === "QUOTE_STALE" || e.code === "QUOTE_EXPIRED") && !requoted) {
          await getQuote();
          setRequoted(true);
          throw new ApiError("QUOTE_STALE", "Prices moved, so we refreshed your quote. Check the new numbers and tap Buy again.", 409);
        }
        throw e;
      }
    },
    submit: (id, signed) => api.orderSubmit(id, { signed_txs: signed }),
  });

  const orderId = flow.result?.order_id;
  const order = useQuery({
    queryKey: ["order", orderId],
    queryFn: () => api.order(orderId!),
    enabled: !!orderId,
    initialData: flow.result ?? undefined,
    refetchInterval: (q) => (q.state.data && ["filled", "failed"].includes(q.state.data.status) ? false : 2500),
  });
  useEffect(() => {
    if (order.data?.status === "filled") qc.invalidateQueries({ queryKey: ["portfolio"] });
  }, [order.data?.status, qc]);

  const price = formatPrice(side === "yes" ? market.yes_price : market.no_price);
  const left = useCountdown(quote?.expires_at);
  const quoteExpired = left !== null && left <= 0;
  const payout = quote ? sharesToNumber(quote.expected_shares) : null;

  if (flow.phase === "done" && order.data) {
    return <OrderResult order={order.data} onAgain={() => { flow.reset(); setQuote(null); }} />;
  }

  return (
    <div className="rounded-[var(--radius-card)] border border-line bg-surface p-5 sm:p-6">
      <div className="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Side">
        {(["yes", "no"] as const).map((s) => {
          const p = formatPrice(s === "yes" ? market.yes_price : market.no_price);
          const active = side === s;
          return (
            <button
              key={s}
              role="radio"
              aria-checked={active}
              onClick={() => setSide(s)}
              className={cn(
                "flex h-16 flex-col items-center justify-center rounded-2xl border-2 transition-colors",
                active ? (s === "yes" ? "border-yes bg-yes-soft text-yes" : "border-no bg-no-soft text-no") : "border-line text-muted hover:border-line-strong",
              )}
            >
              <span className="text-sm font-semibold uppercase tracking-wide">{s}</span>
              <span className="font-display text-lg font-semibold num">{p.cents}</span>
            </button>
          );
        })}
      </div>

      <div className="mt-5">
        <AmountInput value={amount} onChange={setAmount} error={amountErr} />
      </div>

      {quote && !quoteExpired ? (
        <dl className="mt-5 rounded-2xl bg-sunk px-4 py-3">
          <QuoteRow label="Avg price" value={`${formatPrice(quote.avg_price).cents} per share`} />
          <QuoteRow label="Est. shares" value={formatShares(quote.expected_shares)} />
          <QuoteRow label="Price impact" value={formatBps(quote.price_impact_bps)} hint="How much your buy moves the bonding curve" />
          <QuoteRow label="Panta fee" value={formatUsdc(quote.panta_fee_usdc)} />
          <QuoteRow label="Tenor fee" value={Number(quote.our_fee_usdc) === 0 ? "Free" : formatUsdc(quote.our_fee_usdc)} />
          <div className="mt-1 border-t border-line pt-1">
            <QuoteRow strong label={`Payout if ${side.toUpperCase()}`} value={payout != null ? `$${payout.toFixed(2)}` : "–"} />
          </div>
          <p className="mt-1 text-right text-xs text-muted num">Quote valid {Math.max(0, Math.ceil(left ?? 0))}s</p>
        </dl>
      ) : (
        <p className="mt-5 text-sm text-muted">
          {price.value != null ? (
            <>
              At {price.cents}, each share pays <b className="text-ink">$1.00</b> if {side.toUpperCase()} wins.
            </>
          ) : (
            "This market isn't pricing right now."
          )}
        </p>
      )}

      {(quoteErr || flow.error) && (
        <p role="alert" className="mt-4 rounded-xl bg-no-soft px-3 py-2.5 text-sm text-no">{quoteErr ?? flow.error}</p>
      )}

      <div className="mt-5">
        {!wallet.address ? (
          <Button size="lg" className="w-full" onClick={wallet.openModal}>Connect wallet to trade</Button>
        ) : !quote || quoteExpired ? (
          <Button size="lg" className="w-full" onClick={getQuote} loading={quoting} disabled={!amountBase || !!amountErr || !tradable}>
            {quoteExpired ? "Refresh quote" : "Preview order"}
          </Button>
        ) : (
          <Button size="lg" variant={side === "yes" ? "yes" : "no"} className="w-full" onClick={flow.start} loading={flow.phase === "building"}>
            Buy {side.toUpperCase()} · {formatUsdc(quote.amount_usdc)}
          </Button>
        )}
      </div>

      <div className="mt-4 flex items-start justify-between gap-3 text-xs text-muted">
        <p className="flex gap-1.5">
          <Hourglass size={14} className="mt-px shrink-0" aria-hidden="true" /> Held until resolution. No selling before it resolves.
        </p>
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
        error={flow.error}
      />
    </div>
  );
}

function OrderResult({ order, onAgain }: { order: OrderView; onAgain: () => void }) {
  const done = order.status === "filled";
  const failed = order.status === "failed";
  return (
    <div className="rounded-[var(--radius-card)] border border-line bg-surface p-6 text-center" aria-live="polite">
      <div className={cn("mx-auto grid size-14 place-items-center rounded-full", done ? "bg-yes-soft text-yes" : failed ? "bg-no-soft text-no" : "bg-sunk")}>
        {done ? <CheckCircle size={30} weight="fill" /> : failed ? <XCircle size={30} weight="fill" /> : <Spinner className="size-6" />}
      </div>
      <h3 className="mt-4 text-xl font-semibold tracking-tight">{done ? "You're in." : failed ? "Order failed" : "Confirming on Solana…"}</h3>
      <p className="mt-1 text-sm text-muted">
        {done
          ? `${formatShares(order.expected_shares)} ${order.side.toUpperCase()} shares are in your wallet.`
          : failed
            ? `Nothing was bought${order.error_code ? ` (${order.error_code})` : ""}. Your USDC is untouched.`
            : `Status: ${order.status}. This usually takes a few seconds.`}
      </p>
      {order.tx_signature && (
        <a href={`https://solscan.io/tx/${order.tx_signature}`} target="_blank" rel="noreferrer" className="mt-3 inline-flex items-center gap-1 text-xs font-medium underline underline-offset-4">
          <Info size={13} aria-hidden="true" /> View transaction
        </a>
      )}
      <div className="mt-6 flex gap-2">
        <Button variant="secondary" className="flex-1" onClick={onAgain}>Buy more</Button>
        <Link to="/portfolio" className="inline-flex h-11 flex-1 items-center justify-center rounded-full bg-ink text-[15px] font-medium text-paper">Portfolio</Link>
      </div>
    </div>
  );
}

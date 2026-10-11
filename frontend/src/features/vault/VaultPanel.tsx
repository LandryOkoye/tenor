import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";
import { CheckCircle, Coins, Info, XCircle } from "@phosphor-icons/react";
import type { BasketTemplateDetail, VaultBasketView, VaultBuildResponse } from "@/api/types";
import { api } from "@/api/endpoints";
import { Button, Spinner } from "@/components/ui/Button";
import { Badge } from "@/components/ui/primitives";
import { formatShares, formatUsdc, toBaseUnits } from "@/lib/money";
import { useSession } from "@/wallet/session";
import { useTenorWallet } from "@/wallet/useTenorWallet";
import { AmountInput } from "@/features/trade/AmountInput";
import { QuoteRow } from "@/features/trade/QuoteRows";
import { SignReview } from "@/features/trade/SignReview";
import { useVaultTx } from "./useVaultTx";

export const VAULT_STATUS: Record<string, { label: string; tone: "neutral" | "yes" | "warn" | "accent" }> = {
  pending: { label: "Launching", tone: "warn" },
  live: { label: "Open for deposits", tone: "yes" },
  closed: { label: "Closed · awaiting resolution", tone: "neutral" },
  settled: { label: "Settled · redeemable", tone: "accent" },
};

/** Tokenized basket (ADR 0002): deposit USDC, receive an SPL basket token priced at NAV. */
export function VaultPanel({ basket, vault }: { basket: BasketTemplateDetail; vault: VaultBasketView }) {
  const wallet = useTenorWallet();
  const { ensureSession } = useSession();
  const qc = useQueryClient();
  const [amount, setAmount] = useState("25");
  const [built, setBuilt] = useState<VaultBuildResponse | null>(null);
  const base = toBaseUnits(amount);
  const n = base ? Number(amount) : 0;
  const amountErr = !amount ? null : !base ? "Enter a valid amount" : n < 1 || n > 1000 ? "Deposits are $1 to $1,000" : null;

  const { flow, tx, done } = useVaultTx(
    async () => {
      await ensureSession();
      return api.vaultDeposit(vault.template_id, { amount_usdc: base!, slippage_bps: 150 });
    },
    setBuilt,
  );
  useEffect(() => {
    if (!done) return;
    qc.invalidateQueries({ queryKey: ["portfolio"] });
    qc.invalidateQueries({ queryKey: ["vault", basket.slug] });
  }, [done, qc, basket.slug]);
  const s = VAULT_STATUS[vault.status] ?? { label: vault.status, tone: "neutral" as const };

  return (
    <div className="rounded-[var(--radius-card)] border border-line bg-surface p-5 sm:p-6">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="flex items-center gap-2 text-lg font-semibold tracking-tight">
            <Coins size={20} aria-hidden="true" /> Basket token
          </p>
          <p className="mt-0.5 text-sm text-muted">Hold the whole basket as one SPL token, priced at net asset value.</p>
        </div>
        <Badge tone={s.tone}>{s.label}</Badge>
      </div>

      <dl className="mt-4 grid grid-cols-3 gap-3 rounded-2xl bg-sunk p-4 text-sm">
        <div>
          <dt className="text-xs text-muted">NAV / token</dt>
          <dd className="mt-0.5 font-display text-lg font-semibold num">{vault.nav_per_token ? `$${Number(vault.nav_per_token).toFixed(4)}` : "–"}</dd>
        </div>
        <div>
          <dt className="text-xs text-muted">Vault NAV</dt>
          <dd className="mt-0.5 font-medium num">{vault.nav_usdc ? formatUsdc(vault.nav_usdc) : "–"}</dd>
        </div>
        <div>
          <dt className="text-xs text-muted">Supply</dt>
          <dd className="mt-0.5 font-medium num">{formatShares(vault.supply)}</dd>
        </div>
      </dl>
      <p className="mt-2 text-xs text-accent-ink">NAV is read on chain: Tenor analysis.</p>

      {tx && done ? (
        <div className="mt-5 rounded-2xl border border-line p-4 text-center" aria-live="polite">
          {tx.status === "confirmed" ? <CheckCircle size={28} weight="fill" className="mx-auto text-yes" /> : <XCircle size={28} weight="fill" className="mx-auto text-no" />}
          <p className="mt-2 font-medium">{tx.status === "confirmed" ? `Deposited. ${formatShares(tx.tokens)} basket tokens are in your wallet.` : `Deposit failed${tx.error_code ? ` (${tx.error_code})` : ""}. Your USDC is untouched.`}</p>
          <div className="mt-4 flex gap-2">
            <Button variant="secondary" className="flex-1" onClick={() => { flow.reset(); setBuilt(null); }}>Deposit more</Button>
            <Link to="/portfolio" className="inline-flex h-11 flex-1 items-center justify-center rounded-full bg-ink text-[15px] font-medium text-paper">Portfolio</Link>
          </div>
        </div>
      ) : tx ? (
        <p className="mt-5 flex items-center justify-center gap-2 text-sm text-muted" aria-live="polite"><Spinner /> Confirming deposit…</p>
      ) : vault.status === "live" ? (
        <>
          <div className="mt-5">
            <AmountInput label="Deposit" value={amount} onChange={setAmount} min={1} max={1000} error={amountErr} />
          </div>
          {flow.error && <p role="alert" className="mt-3 rounded-xl bg-no-soft px-3 py-2.5 text-sm text-no">{flow.error}</p>}
          {!wallet.address ? (
            <Button size="lg" className="mt-4 w-full" onClick={wallet.openModal}>Connect wallet</Button>
          ) : (
            <Button size="lg" className="mt-4 w-full" onClick={flow.start} loading={flow.phase === "building"} disabled={!base || !!amountErr}>
              Mint basket tokens
            </Button>
          )}
        </>
      ) : (
        <p className="mt-5 flex gap-2 rounded-xl bg-sunk px-3 py-2.5 text-sm text-muted">
          <Info size={18} className="shrink-0" aria-hidden="true" />
          {vault.status === "settled" ? "Every market has resolved. Redeem your basket tokens for USDC from your Portfolio." : vault.status === "closed" ? "Deposits are closed. Tokens become redeemable for USDC once every market resolves." : "This basket token is launching. Deposits open shortly."}
        </p>
      )}

      <SignReview
        open={flow.phase === "review" || flow.phase === "signing"}
        onClose={flow.cancel}
        onSign={flow.sign}
        summary={flow.built?.summary ?? null}
        expiresAt={null}
        txCount={flow.built?.transactions.length ?? 1}
        signing={flow.phase === "signing"}
        title="Review deposit"
        error={flow.error}
        extra={
          built && (
            <dl className="rounded-2xl border border-line px-4 py-2">
              <QuoteRow strong label="You receive (est.)" value={`${formatShares(built.expected_tokens)} tokens`} />
              <QuoteRow label="Minimum, or the tx reverts" value={`${formatShares(built.min_tokens_out)} tokens`} />
            </dl>
          )
        }
      />
    </div>
  );
}

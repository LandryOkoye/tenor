import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";
import { ArrowUpRight, Coins } from "@phosphor-icons/react";
import type { PortfolioVaultHolding } from "@/api/types";
import { api } from "@/api/endpoints";
import { Button, Spinner } from "@/components/ui/Button";
import { Badge, Card } from "@/components/ui/primitives";
import { cn } from "@/lib/cn";
import { formatShares, formatUsdc, fromBaseUnits } from "@/lib/money";
import { SignReview } from "@/features/trade/SignReview";
import { useVaultTx } from "./useVaultTx";
import { VAULT_STATUS } from "./VaultPanel";

/** Basket tokens in the wallet; redeemable for USDC once the vault is settled. */
export function VaultHoldingRow({ h }: { h: PortfolioVaultHolding }) {
  const qc = useQueryClient();
  const { flow, tx, done } = useVaultTx(() => api.vaultRedeem(h.template_id, { tokens: h.tokens }));
  useEffect(() => {
    if (done) qc.invalidateQueries({ queryKey: ["portfolio"] });
  }, [done, qc]);

  const pnl = h.value_usdc != null ? fromBaseUnits(h.value_usdc) - fromBaseUnits(h.cost_basis_usdc) : null;
  const s = VAULT_STATUS[h.status] ?? { label: h.status, tone: "neutral" as const };

  return (
    <Card as="article" className="p-5 sm:p-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <p className="label-mono flex items-center gap-1.5 text-muted"><Coins size={14} aria-hidden="true" /> Basket token</p>
          <Link to={`/baskets/${h.template_slug}`} className="group mt-1 inline-flex items-center gap-1 text-lg font-semibold tracking-tight hover:underline underline-offset-4">
            {h.template_title} <ArrowUpRight size={16} className="opacity-50 group-hover:opacity-100" aria-hidden="true" />
          </Link>
          <div className="mt-1.5 flex flex-wrap items-center gap-2 text-sm text-muted num">
            <Badge tone={s.tone}>{s.label}</Badge>
            <span>{formatShares(h.tokens)} tokens</span>
          </div>
        </div>
        <div className="sm:text-right">
          <p className="font-display text-2xl font-semibold tracking-tight num">{h.value_usdc != null ? formatUsdc(h.value_usdc) : "–"}</p>
          <p className={cn("text-sm num", pnl == null ? "text-muted" : pnl >= 0 ? "text-yes" : "text-no")}>
            {pnl == null ? "NAV unavailable right now" : `${pnl >= 0 ? "+" : "−"}$${Math.abs(pnl).toFixed(2)} on ${formatUsdc(h.cost_basis_usdc)}`}
          </p>
        </div>
      </div>
      {h.status === "settled" && (
        <div className="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-2xl bg-accent-soft p-4">
          <p className="text-sm">Every market has resolved. Redeem your tokens for USDC at the final NAV.</p>
          {tx && !done ? (
            <span className="inline-flex items-center gap-2 text-sm"><Spinner /> Redeeming…</span>
          ) : tx && done ? (
            <Badge tone={tx.status === "confirmed" ? "yes" : "no"}>{tx.status === "confirmed" ? "Redeemed" : `Failed${tx.error_code ? ` · ${tx.error_code}` : ""}`}</Badge>
          ) : (
            <Button size="sm" variant="primary" onClick={flow.start} loading={flow.phase === "building"}>Redeem for USDC</Button>
          )}
        </div>
      )}
      {flow.error && flow.phase === "idle" && <p role="alert" className="mt-3 text-sm text-no">{flow.error}</p>}
      <SignReview
        open={flow.phase === "review" || flow.phase === "signing"}
        onClose={flow.cancel}
        onSign={flow.sign}
        summary={flow.built?.summary ?? null}
        expiresAt={null}
        txCount={flow.built?.transactions.length ?? 1}
        signing={flow.phase === "signing"}
        title="Redeem basket tokens"
        error={flow.error}
      />
    </Card>
  );
}

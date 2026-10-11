import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { CheckCircle } from "@phosphor-icons/react";
import type { ClaimsSubmitResponse } from "@/api/types";
import { api } from "@/api/endpoints";
import { Button, type ButtonProps } from "@/components/ui/Button";
import { SignReview } from "@/features/trade/SignReview";
import { useSignFlow } from "@/features/trade/useSignFlow";

/**
 * Claim winnings for one or many markets (brief §7.7). If "claim all" doesn't
 * fit one transaction, the backend returns the rest and we offer another round.
 */
export function ClaimButton({ marketIds, label, ...rest }: { marketIds: string[]; label: string } & Omit<ButtonProps, "onClick">) {
  const qc = useQueryClient();
  const pending = useRef<string[]>(marketIds);
  const [remaining, setRemaining] = useState<string[]>([]);

  const flow = useSignFlow<ClaimsSubmitResponse>({
    build: async () => {
      const b = await api.claimsBuild({ market_ids: pending.current });
      setRemaining(b.remaining_market_ids);
      return { id: b.claim_id, transactions: b.transactions, summary: b.summary, expires_at: null };
    },
    submit: async (id, signed) => {
      const r = await api.claimsSubmit({ claim_id: id, signed_txs: signed });
      qc.invalidateQueries({ queryKey: ["portfolio"] });
      qc.invalidateQueries({ queryKey: ["activity"] });
      return r;
    },
  });

  if (flow.phase === "done") {
    if (remaining.length > 0) {
      return (
        <Button {...rest} variant="accent" onClick={() => { pending.current = remaining; flow.start(); }}>
          Claim remaining {remaining.length}
        </Button>
      );
    }
    return (
      <span className="inline-flex h-9 items-center gap-1.5 rounded-full bg-yes-soft px-3.5 text-sm font-medium text-yes" role="status">
        <CheckCircle size={16} weight="fill" aria-hidden="true" /> {flow.result?.status === "confirmed" ? "Claimed" : "Claim sent"}
      </span>
    );
  }

  return (
    <>
      <Button {...rest} onClick={flow.start} loading={flow.phase === "building"}>{label}</Button>
      {flow.error && flow.phase === "idle" && <p role="alert" className="mt-1 text-xs text-no">{flow.error}</p>}
      <SignReview
        open={flow.phase === "review" || flow.phase === "signing"}
        onClose={flow.cancel}
        onSign={flow.sign}
        summary={flow.built?.summary ?? null}
        expiresAt={null}
        txCount={flow.built?.transactions.length ?? 1}
        signing={flow.phase === "signing"}
        title="Claim winnings"
        error={flow.error}
      />
    </>
  );
}

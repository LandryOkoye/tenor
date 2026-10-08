import { useCallback, useState } from "react";
import type { TxSummary } from "@/api/types";
import { friendlyError } from "@/api/client";
import { useTenorWallet } from "@/wallet/useTenorWallet";

export type Built = { id: string; transactions: string[]; summary: TxSummary; expires_at: string | null };
type Phase = "idle" | "building" | "review" | "signing" | "done";

/**
 * build → review (plain summary) → wallet signs (no broadcast) → backend submits.
 * Shared by single buys, basket buys and claims.
 */
export function useSignFlow<R>({ build, submit }: { build: () => Promise<Built>; submit: (id: string, signed: string[]) => Promise<R> }) {
  const wallet = useTenorWallet();
  const [phase, setPhase] = useState<Phase>("idle");
  const [built, setBuilt] = useState<Built | null>(null);
  const [result, setResult] = useState<R | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rawError, setRawError] = useState<unknown>(null);

  const start = useCallback(async () => {
    setError(null);
    setRawError(null);
    setPhase("building");
    try {
      const b = await build();
      setBuilt(b);
      setPhase("review");
    } catch (e) {
      setRawError(e);
      setError(friendlyError(e));
      setPhase("idle");
    }
  }, [build]);

  const sign = useCallback(async () => {
    if (!built) return;
    setError(null);
    setPhase("signing");
    try {
      const signed = await wallet.signTransactions(built.transactions);
      const r = await submit(built.id, signed);
      setResult(r);
      setPhase("done");
    } catch (e) {
      setRawError(e);
      setError(friendlyError(e));
      setPhase("review");
    }
  }, [built, submit, wallet]);

  const cancel = useCallback(() => {
    setPhase("idle");
    setBuilt(null);
  }, []);

  const reset = useCallback(() => {
    setPhase("idle");
    setBuilt(null);
    setResult(null);
    setError(null);
    setRawError(null);
  }, []);

  return { phase, built, result, error, rawError, start, sign, cancel, reset, setError };
}

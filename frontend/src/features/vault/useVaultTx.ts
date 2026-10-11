import { useQuery } from "@tanstack/react-query";
import type { VaultBuildResponse, VaultTxView } from "@/api/types";
import { api } from "@/api/endpoints";
import { useSignFlow } from "@/features/trade/useSignFlow";

const DONE = ["confirmed", "failed"];

/** build → sign → `/v1/vault/txs/{id}/submit` → poll while `submitted` (docs/api.md). */
export function useVaultTx(build: () => Promise<VaultBuildResponse>, onBuilt?: (b: VaultBuildResponse) => void) {
  const flow = useSignFlow<VaultTxView>({
    build: async () => {
      const b = await build();
      onBuilt?.(b);
      return { id: b.tx_id, transactions: b.transactions, summary: b.summary, expires_at: null };
    },
    submit: (id, signed) => api.vaultSubmit(id, { signed_txs: signed }),
  });
  const txId = flow.result?.tx_id;
  const tx = useQuery({
    queryKey: ["vault-tx", txId],
    queryFn: () => api.vaultTx(txId!),
    enabled: !!txId,
    initialData: flow.result ?? undefined,
    refetchInterval: (q) => (q.state.data && DONE.includes(q.state.data.status) ? false : 2500),
  });
  return { flow, tx: tx.data ?? null, done: !!tx.data && DONE.includes(tx.data.status) };
}

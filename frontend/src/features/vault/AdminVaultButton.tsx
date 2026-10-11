import type { VaultBuildResponse } from "@/api/types";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/primitives";
import { SignReview } from "@/features/trade/SignReview";
import { useVaultTx } from "./useVaultTx";

/** Admin vault action (tokenize / close / settle): build → sign → submit, admin pays rent. */
export function AdminVaultButton({ label, build }: { label: string; build: () => Promise<VaultBuildResponse> }) {
  const { flow, tx, done } = useVaultTx(build);
  if (tx) return <Badge tone={!done ? "warn" : tx.status === "confirmed" ? "yes" : "no"}>{!done ? `${label}…` : tx.status === "confirmed" ? `${label}: done` : `${label}: failed`}</Badge>;
  return (
    <>
      <Button size="sm" variant="secondary" onClick={flow.start} loading={flow.phase === "building"} title={flow.error ?? undefined}>
        {label}
      </Button>
      {flow.error && flow.phase === "idle" && <span role="alert" className="text-xs text-no">{flow.error}</span>}
      <SignReview
        open={flow.phase === "review" || flow.phase === "signing"}
        onClose={flow.cancel}
        onSign={flow.sign}
        summary={flow.built?.summary ?? null}
        expiresAt={null}
        txCount={flow.built?.transactions.length ?? 1}
        signing={flow.phase === "signing"}
        title={label}
        error={flow.error}
      />
    </>
  );
}

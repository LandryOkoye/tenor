import { useEffect, useState, type ReactNode } from "react";
import { Info, LockSimple, Timer } from "@phosphor-icons/react";
import type { TxSummary } from "@/api/types";
import { Dialog } from "@/components/ui/Dialog";
import { Button } from "@/components/ui/Button";
import { formatUsdc } from "@/lib/money";
import { secondsUntil } from "@/lib/time";
import { PoweredByPanta } from "@/components/ui/primitives";

/**
 * Plain summary shown before every signature (brief §12): what is bought,
 * every fee line, the buy-only notice, and how long the build stays valid.
 */
export function SignReview({
  open,
  onClose,
  onSign,
  summary,
  expiresAt,
  txCount,
  signing,
  title = "Review and sign",
  signLabel = "Approve in wallet",
  error,
  extra,
}: {
  open: boolean;
  onClose: () => void;
  onSign: () => void;
  summary: TxSummary | null;
  expiresAt: string | null;
  txCount: number;
  signing: boolean;
  title?: string;
  /** Confirm button text; matches the action the panel button previewed. */
  signLabel?: string;
  error?: string | null;
  /** Extra disclosure rows (e.g. expected basket tokens) shown above the fees. */
  extra?: ReactNode;
}) {
  const left = useCountdown(expiresAt);
  const expired = left !== null && left <= 0;
  return (
    <Dialog open={open} onClose={onClose} title={title} dismissable={!signing}>
      {summary && (
        <div className="space-y-5">
          <ul className="space-y-2 rounded-2xl bg-sunk p-4 text-sm">
            {summary.summary_lines.map((l, i) => (
              <li key={i} className="flex gap-2.5">
                <span className="mt-[7px] size-1.5 shrink-0 rounded-full bg-accent" aria-hidden="true" />
                <span>{l}</span>
              </li>
            ))}
          </ul>
          {extra}
          <dl className="space-y-2 text-sm">
            <Row label="Tenor fee" value={formatUsdc(summary.our_fee_usdc)} />
            <Row label="Panta protocol fees (est.)" value={formatUsdc(summary.panta_fees_usdc)} />
            <div className="border-t border-line pt-2">
              <Row label="Total you pay" value={formatUsdc(summary.total_usdc)} strong />
            </div>
          </dl>
          {summary.notices.length > 0 && (
            <ul className="space-y-2">
              {summary.notices.map((n, i) => (
                <li key={i} className="flex gap-2 rounded-xl bg-warn-soft px-3 py-2.5 text-sm text-ink">
                  <Info size={18} className="mt-px shrink-0 text-warn" aria-hidden="true" /> {n}
                </li>
              ))}
            </ul>
          )}
          {txCount > 1 && (
            <p className="rounded-xl border border-line px-3 py-2.5 text-sm">
              This needs <b>{txCount} signatures</b>. If prices move between them, some parts may fail. You'll see exactly which legs filled.
            </p>
          )}
          {error && (
            <p role="alert" className="rounded-xl bg-no-soft px-3 py-2.5 text-sm text-no">
              {error}
            </p>
          )}
          <div className="flex items-center justify-between text-xs text-muted">
            <span className="inline-flex items-center gap-1.5 num">
              <Timer size={14} aria-hidden="true" />
              {left === null ? "" : expired ? "Expired: close and requote" : `Valid for ${Math.ceil(left)}s`}
            </span>
            <PoweredByPanta />
          </div>
          <Button size="lg" className="w-full" onClick={onSign} loading={signing} disabled={expired}>
            <LockSimple size={18} weight="bold" aria-hidden="true" />
            {signing ? "Waiting for your wallet…" : signLabel}
          </Button>
        </div>
      )}
    </Dialog>
  );
}

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <dt className={strong ? "font-medium" : "text-muted"}>{label}</dt>
      <dd className={`num ${strong ? "font-display text-lg font-semibold" : ""}`}>{value}</dd>
    </div>
  );
}

export function useCountdown(iso: string | null | undefined): number | null {
  const [left, setLeft] = useState<number | null>(() => secondsUntil(iso));
  useEffect(() => {
    setLeft(secondsUntil(iso));
    if (!iso) return;
    const t = setInterval(() => setLeft(secondsUntil(iso)), 1000);
    return () => clearInterval(t);
  }, [iso]);
  return left;
}

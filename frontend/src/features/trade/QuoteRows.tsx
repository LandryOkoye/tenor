import type { ReactNode } from "react";

export function QuoteRow({ label, value, hint, strong }: { label: ReactNode; value: ReactNode; hint?: string; strong?: boolean }) {
  return (
    <div className="flex items-baseline justify-between gap-4 py-1.5 text-sm">
      <dt className={strong ? "font-medium" : "text-muted"} title={hint}>{label}</dt>
      <dd className={`text-right num ${strong ? "font-display text-base font-semibold" : ""}`}>{value}</dd>
    </div>
  );
}

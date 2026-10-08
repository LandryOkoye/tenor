import { useId } from "react";
import { cn } from "@/lib/cn";

const QUICK = ["5", "10", "25", "50"];

export function AmountInput({ value, onChange, min, max, error, label = "Amount" }: { value: string; onChange: (v: string) => void; min?: number; max?: number; error?: string | null; label?: string }) {
  const id = useId();
  return (
    <div>
      <label htmlFor={id} className="text-sm font-medium">{label}</label>
      <div className={cn("mt-2 flex h-14 items-center rounded-2xl border bg-surface px-4 focus-within:border-ink", error ? "border-no" : "border-line")}>
        <span className="font-display text-2xl font-semibold text-muted">$</span>
        <input
          id={id}
          inputMode="decimal"
          autoComplete="off"
          value={value}
          onChange={(e) => onChange(e.target.value.replace(/[^\d.]/g, "").replace(/(\..*)\./g, "$1"))}
          placeholder="0.00"
          aria-invalid={!!error}
          aria-describedby={`${id}-help`}
          className="w-full bg-transparent px-1.5 font-display text-2xl font-semibold tracking-tight outline-none placeholder:text-faint num"
        />
        <span className="label-mono text-muted">USDC</span>
      </div>
      <div className="mt-2 flex gap-2">
        {QUICK.map((q) => (
          <button key={q} type="button" onClick={() => onChange(q)} className={cn("h-9 flex-1 rounded-full border text-sm font-medium num transition-colors", value === q ? "border-ink bg-ink text-paper" : "border-line bg-surface hover:border-line-strong")}>
            ${q}
          </button>
        ))}
      </div>
      <p id={`${id}-help`} className={cn("mt-2 text-xs", error ? "text-no" : "text-muted")} role={error ? "alert" : undefined}>
        {error ?? (min != null && max != null ? `Min $${min} · max $${max}` : "Settled in USDC from your wallet")}
      </p>
    </div>
  );
}

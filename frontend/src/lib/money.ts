// Money is USDC base units (6 decimals) as integer strings on the wire.
// Convert only at the UI boundary, never with floats for the stored value.

const DECIMALS = 6n;
const UNIT = 10n ** DECIMALS;

/** "20.5" → "20500000". Returns null for invalid / non-positive input. */
export function toBaseUnits(human: string): string | null {
  const s = human.trim();
  if (!/^\d+(\.\d{0,6})?$/.test(s)) return null;
  const [whole, frac = ""] = s.split(".");
  const base = BigInt(whole) * UNIT + BigInt((frac + "000000").slice(0, 6));
  return base > 0n ? base.toString() : null;
}

/** "20500000" → 20.5 (display only). */
export function fromBaseUnits(base: string | null | undefined): number {
  if (!base) return 0;
  try {
    const b = BigInt(base);
    const neg = b < 0n;
    const abs = neg ? -b : b;
    const n = Number(abs / UNIT) + Number(abs % UNIT) / 1e6;
    return neg ? -n : n;
  } catch {
    return Number(base) / 1e6 || 0;
  }
}

const usd = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", minimumFractionDigits: 2, maximumFractionDigits: 2 });
const compact = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 });

/** Base units → "$20.50" (USDC shown as dollars). */
export function formatUsdc(base: string | null | undefined, opts: { sign?: boolean } = {}): string {
  const n = fromBaseUnits(base);
  const s = usd.format(Math.abs(n));
  if (opts.sign) return (n > 0 ? "+" : n < 0 ? "−" : "") + s;
  return n < 0 ? "−" + s : s;
}

/** Base units → "$1.2K". */
export function formatUsdcCompact(base: string | null | undefined): string {
  const n = fromBaseUnits(base);
  return n < 1000 ? usd.format(n) : "$" + compact.format(n);
}

/** Decimal price string "0.62" → { cents: "62¢", pct: "62%" }. */
export function formatPrice(price: string | null | undefined): { cents: string; pct: string; value: number | null } {
  if (price == null || price === "") return { cents: "–", pct: "–", value: null };
  const v = Number(price);
  if (!Number.isFinite(v)) return { cents: "–", pct: "–", value: null };
  const c = v * 100;
  const cents = c < 1 && c > 0 ? "<1¢" : `${c >= 10 ? Math.round(c) : c.toFixed(1).replace(/\.0$/, "")}¢`;
  return { cents, pct: `${Math.round(c)}%`, value: v };
}

/** Shares (and basket tokens) are integer strings with 6 decimals, like USDC. */
export function sharesToNumber(base: string | null | undefined): number {
  return fromBaseUnits(base);
}

export function formatShares(base: string | null | undefined): string {
  if (!base) return "–";
  return sharesToNumber(base).toLocaleString("en-US", { maximumFractionDigits: 2 });
}

export function formatBps(bps: number | null | undefined): string {
  if (bps == null) return "–";
  return `${(bps / 100).toFixed(bps % 100 === 0 ? 0 : 2)}%`;
}

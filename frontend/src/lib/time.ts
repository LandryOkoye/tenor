const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

export function formatDate(iso: string | null | undefined, withTime = false): string {
  if (!iso) return "–";
  const d = new Date(iso);
  return d.toLocaleString("en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
    ...(withTime ? { hour: "2-digit", minute: "2-digit" } : {}),
  });
}

/** "in 3 days", "2 hours ago". */
export function relative(iso: string | null | undefined): string {
  if (!iso) return "–";
  const diff = (new Date(iso).getTime() - Date.now()) / 1000;
  return relativeSeconds(diff);
}

export function relativeSeconds(diff: number): string {
  const abs = Math.abs(diff);
  if (abs < 60) return rtf.format(Math.round(diff), "second");
  if (abs < 3600) return rtf.format(Math.round(diff / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(diff / 3600), "hour");
  if (abs < 86400 * 45) return rtf.format(Math.round(diff / 86400), "day");
  return rtf.format(Math.round(diff / (86400 * 30)), "month");
}

/** Compact countdown "3d 4h" / "12m" from seconds. */
export function shortDuration(seconds: number | null | undefined): string {
  if (seconds == null) return "–";
  const s = Math.max(0, Math.floor(seconds));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${s}s`;
}

export function secondsUntil(iso: string | null | undefined): number | null {
  if (!iso) return null;
  return (new Date(iso).getTime() - Date.now()) / 1000;
}

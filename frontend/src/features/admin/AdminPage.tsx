import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { MagnifyingGlass, Plus, ShieldWarning, Trash, Warning } from "@phosphor-icons/react";
import type { AdminBasketView, AdminPantaMarket, Side } from "@/api/types";
import { api } from "@/api/endpoints";
import { ApiError, friendlyError } from "@/api/client";
import { qk, useBaskets } from "@/api/queries";
import { Badge, Card, EmptyState } from "@/components/ui/primitives";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/cn";
import { formatPrice, formatUsdcCompact, toBaseUnits } from "@/lib/money";
import { relative, secondsUntil } from "@/lib/time";
import { useSession } from "@/wallet/session";
import { AdminVaultButton } from "@/features/vault/AdminVaultButton";
import { useTenorWallet } from "@/wallet/useTenorWallet";

const MAX_LEGS = 4;
type Leg = { market: AdminPantaMarket; side: Side; weight: string };
const slugify = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 60);

export default function AdminPage() {
  const wallet = useTenorWallet();
  const { session, ensureSession, signingIn } = useSession();
  const [err, setErr] = useState<string | null>(null);

  if (!wallet.address || !session) {
    return (
      <div className="mx-auto max-w-lg py-16">
        <EmptyState
          icon={<ShieldWarning size={24} />}
          title="Admin: basket curation"
          body="Only allowlisted admin wallets can curate baskets. Connect and sign in with an admin wallet."
          action={
            <div className="flex flex-col items-center">
              <Button loading={signingIn} onClick={async () => { setErr(null); try { await ensureSession(); } catch (e) { setErr(friendlyError(e)); } }}>
                {wallet.address ? "Sign in" : "Connect wallet"}
              </Button>
              {err && <p role="alert" className="mt-3 text-sm text-no">{err}</p>}
            </div>
          }
        />
      </div>
    );
  }
  return <Curator />;
}

function Curator() {
  const qc = useQueryClient();
  const [q, setQ] = useState("");
  const [debounced, setDebounced] = useState("");
  useEffect(() => {
    const t = setTimeout(() => setDebounced(q), 300);
    return () => clearTimeout(t);
  }, [q]);
  const search = useQuery({ queryKey: ["admin-markets", debounced], queryFn: () => api.adminMarkets(debounced), enabled: debounced.length > 1 });

  const [legs, setLegs] = useState<Leg[]>([]);
  const [title, setTitle] = useState("");
  const [slug, setSlug] = useState("");
  const [slugTouched, setSlugTouched] = useState(false);
  const [description, setDescription] = useState("");
  const [category, setCategory] = useState("");
  const [minBuy, setMinBuy] = useState("5");
  const [maxBuy, setMaxBuy] = useState("500");
  const [saving, setSaving] = useState(false);
  const [formErr, setFormErr] = useState<string | null>(null);
  const [draft, setDraft] = useState<AdminBasketView | null>(null);

  useEffect(() => {
    if (!slugTouched) setSlug(slugify(title));
  }, [title, slugTouched]);

  const weightSum = legs.reduce((s, l) => s + (Number(l.weight) || 0), 0);
  const issues = useMemo(() => {
    const out: string[] = [];
    if (legs.length < 2) out.push("Add at least 2 legs.");
    if (Math.abs(weightSum - 100) > 0.001) out.push(`Weights add up to ${weightSum}%, not 100%.`);
    if (!title.trim()) out.push("Add a title.");
    if (!slug) out.push("Add a slug.");
    if (!description.trim()) out.push("Write the thesis.");
    if (!toBaseUnits(minBuy) || !toBaseUnits(maxBuy) || Number(minBuy) > Number(maxBuy)) out.push("Check min and max buy.");
    return out;
  }, [legs.length, weightSum, title, slug, description, minBuy, maxBuy]);

  const addLeg = (m: AdminPantaMarket) => {
    if (legs.length >= MAX_LEGS || legs.some((l) => l.market.id === m.id)) return;
    const next = [...legs, { market: m, side: "yes" as Side, weight: "0" }];
    const even = Math.floor(100 / next.length);
    setLegs(next.map((l, i) => ({ ...l, weight: String(i === next.length - 1 ? 100 - even * (next.length - 1) : even) })));
  };

  const body = () => ({
    slug,
    title: title.trim(),
    description: description.trim(),
    category: category.trim() || null,
    cover_url: null,
    min_buy_usdc: toBaseUnits(minBuy)!,
    max_buy_usdc: toBaseUnits(maxBuy)!,
    legs: legs.map((l) => ({ market_id: l.market.id, side: l.side, weight_bps: Math.round(Number(l.weight) * 100) })),
  });

  const save = async () => {
    setFormErr(null);
    setSaving(true);
    try {
      const v = draft ? await api.adminUpdate(draft.id, body()) : await api.adminCreate(body());
      setDraft(v);
    } catch (e) {
      setFormErr(e instanceof ApiError && e.status === 403 ? "This wallet isn't on the admin allowlist." : friendlyError(e));
    } finally {
      setSaving(false);
    }
  };

  const act = async (fn: () => Promise<AdminBasketView>) => {
    setFormErr(null);
    setSaving(true);
    try {
      setDraft(await fn());
      qc.invalidateQueries({ queryKey: qk.baskets });
    } catch (e) {
      setFormErr(friendlyError(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <p className="label-mono text-muted">Admin</p>
      <h1 className="mt-2 text-4xl font-semibold tracking-[-0.04em]">Curate a basket</h1>
      <p className="mt-2 max-w-xl text-muted">Pick 2–{MAX_LEGS} live markets, choose a side and weight for each, write the thesis, then publish. The server re-checks liquidity and close times on publish.</p>

      <div className="mt-8 grid gap-6 lg:grid-cols-[1fr_1.1fr]">
        <Card className="p-5 sm:p-6">
          <h2 className="font-semibold">1 · Find markets</h2>
          <label className="relative mt-3 block">
            <span className="sr-only">Search Panta markets</span>
            <MagnifyingGlass size={18} className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted" aria-hidden="true" />
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search Panta markets" className="h-12 w-full rounded-full border border-line bg-surface pl-11 pr-4 outline-none focus:border-ink" />
          </label>
          <ul className="mt-3 max-h-[480px] space-y-2 overflow-y-auto">
            {search.isFetching && <li className="py-6 text-center text-sm text-muted">Searching…</li>}
            {search.isError && <li className="py-6 text-center text-sm text-no">{friendlyError(search.error)}</li>}
            {search.data?.map((m) => {
              const soon = (secondsUntil(m.close_time) ?? Infinity) < 86400;
              const added = legs.some((l) => l.market.id === m.id);
              return (
                <li key={m.id} className="rounded-2xl border border-line p-3.5">
                  <div className="flex items-start gap-3">
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-medium leading-snug">{m.title}</p>
                      <p className="mt-1 flex flex-wrap gap-x-3 text-xs text-muted num">
                        <span>YES {formatPrice(m.yes_price).cents}</span>
                        <span>{m.volume_usdc ? formatUsdcCompact(m.volume_usdc) : "no"} vol</span>
                        <span className={soon ? "text-no" : ""}>closes {relative(m.close_time)}</span>
                        <span>{m.status}</span>
                      </p>
                    </div>
                    <Button size="sm" variant="secondary" onClick={() => addLeg(m)} disabled={added || legs.length >= MAX_LEGS} aria-label={`Add ${m.title}`}>
                      <Plus size={14} aria-hidden="true" /> {added ? "Added" : "Add"}
                    </Button>
                  </div>
                  {soon && <p className="mt-2 flex items-center gap-1 text-xs text-no"><Warning size={13} aria-hidden="true" /> Closes within 24h: won't pass publish checks</p>}
                </li>
              );
            })}
            {!search.isFetching && debounced.length > 1 && search.data?.length === 0 && <li className="py-6 text-center text-sm text-muted">No markets found.</li>}
          </ul>
        </Card>

        <Card className="p-5 sm:p-6">
          <h2 className="font-semibold">2 · Legs and weights</h2>
          {legs.length === 0 ? (
            <p className="mt-3 rounded-2xl bg-sunk p-4 text-sm text-muted">No legs yet. Add markets from the search.</p>
          ) : (
            <ul className="mt-3 space-y-2">
              {legs.map((l, i) => (
                <li key={l.market.id} className="flex flex-wrap items-center gap-2 rounded-2xl border border-line p-3">
                  <span className="min-w-0 flex-1 text-sm font-medium">{l.market.title}</span>
                  <div className="flex rounded-full border border-line p-0.5" role="radiogroup" aria-label="Side">
                    {(["yes", "no"] as const).map((s) => (
                      <button key={s} role="radio" aria-checked={l.side === s} onClick={() => setLegs(legs.map((x, j) => (j === i ? { ...x, side: s } : x)))} className={cn("h-8 rounded-full px-3 text-xs font-semibold uppercase", l.side === s ? (s === "yes" ? "bg-yes text-white" : "bg-no text-white") : "text-muted")}>
                        {s}
                      </button>
                    ))}
                  </div>
                  <label className="flex items-center gap-1 text-sm">
                    <span className="sr-only">Weight percent</span>
                    <input inputMode="decimal" value={l.weight} onChange={(e) => setLegs(legs.map((x, j) => (j === i ? { ...x, weight: e.target.value.replace(/[^\d.]/g, "") } : x)))} className="h-9 w-16 rounded-lg border border-line px-2 text-right num outline-none focus:border-ink" />%
                  </label>
                  <button onClick={() => setLegs(legs.filter((_, j) => j !== i))} className="grid size-9 place-items-center rounded-full text-muted hover:bg-no-soft hover:text-no" aria-label={`Remove ${l.market.title}`}>
                    <Trash size={16} aria-hidden="true" />
                  </button>
                </li>
              ))}
              <li className={cn("text-right text-sm num", Math.abs(weightSum - 100) > 0.001 ? "text-no" : "text-yes")}>Total {weightSum}%</li>
            </ul>
          )}

          <h2 className="mt-6 font-semibold">3 · Details</h2>
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            <Field label="Title"><input value={title} onChange={(e) => setTitle(e.target.value)} className={inputCls} /></Field>
            <Field label="Slug"><input value={slug} onChange={(e) => { setSlugTouched(true); setSlug(slugify(e.target.value)); }} className={cn(inputCls, "font-mono text-sm")} /></Field>
            <Field label="Category"><input value={category} onChange={(e) => setCategory(e.target.value)} placeholder="Football" className={inputCls} /></Field>
            <div className="grid grid-cols-2 gap-3">
              <Field label="Min buy ($)"><input inputMode="decimal" value={minBuy} onChange={(e) => setMinBuy(e.target.value)} className={cn(inputCls, "num")} /></Field>
              <Field label="Max buy ($)"><input inputMode="decimal" value={maxBuy} onChange={(e) => setMaxBuy(e.target.value)} className={cn(inputCls, "num")} /></Field>
            </div>
            <Field label="Thesis" className="sm:col-span-2"><textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={4} className={cn(inputCls, "h-auto py-3")} /></Field>
          </div>

          {issues.length > 0 && (
            <ul className="mt-4 space-y-1 text-sm text-muted">
              {issues.map((i) => <li key={i}>· {i}</li>)}
            </ul>
          )}
          {formErr && <p role="alert" className="mt-4 rounded-xl bg-no-soft px-3 py-2.5 text-sm text-no">{formErr}</p>}

          <div className="mt-5 flex flex-wrap gap-2">
            <Button onClick={save} loading={saving} disabled={issues.length > 0}>{draft ? "Save changes" : "Save draft"}</Button>
            {draft && draft.status === "draft" && <Button variant="accent" onClick={() => act(() => api.adminPublish(draft.id))} loading={saving}>Publish</Button>}
            {draft && draft.status === "published" && <Button variant="secondary" onClick={() => act(() => api.adminClose(draft.id))} loading={saving}>Close for new buys</Button>}
          </div>

          {draft && (
            <div className="mt-5 rounded-2xl bg-sunk p-4 text-sm">
              <p className="flex items-center gap-2 font-medium">Saved <Badge tone={draft.status === "published" ? "yes" : "neutral"}>{draft.status}</Badge> <span className="font-mono text-xs text-muted">{draft.id}</span></p>
              {draft.warnings.length > 0 && (
                <ul className="mt-2 space-y-1">
                  {draft.warnings.map((w) => <li key={w} className="flex gap-1.5 text-warn"><Warning size={15} className="mt-0.5 shrink-0" aria-hidden="true" /> {w}</li>)}
                </ul>
              )}
            </div>
          )}
        </Card>
      </div>

      <Published onClose={(id) => act(() => api.adminClose(id))} />
    </div>
  );
}

function Published({ onClose }: { onClose: (id: string) => void }) {
  const { data } = useBaskets();
  if (!data?.length) return null;
  return (
    <Card className="mt-6 p-5 sm:p-6">
      <h2 className="font-semibold">Published baskets</h2>
      <p className="mt-1 text-sm text-muted">Tokenize mints an on-chain basket token (max 3 legs, your wallet pays vault rent). Close stops deposits; settle opens redemptions once every market resolves.</p>
      <ul className="mt-3 divide-y divide-line">
        {data.map((b) => (
          <li key={b.id} className="flex flex-wrap items-center gap-2 py-3">
            <span className="min-w-0 flex-1 basis-full font-medium sm:basis-auto">{b.title}</span>
            <span className="text-sm text-muted">{b.leg_count} legs</span>
            {b.status === "published" ? <Button size="sm" variant="secondary" onClick={() => onClose(b.id)}>Close</Button> : <Badge>{b.status}</Badge>}
            {b.leg_count <= 3 && <AdminVaultButton label="Tokenize" build={() => api.adminTokenize(b.id)} />}
            <AdminVaultButton label="Vault close" build={() => api.adminVaultClose(b.id)} />
            <AdminVaultButton label="Vault settle" build={() => api.adminVaultSettle(b.id)} />
          </li>
        ))}
      </ul>
    </Card>
  );
}

const inputCls = "h-11 w-full rounded-xl border border-line bg-surface px-3.5 outline-none focus:border-ink";

function Field({ label, children, className }: { label: string; children: React.ReactNode; className?: string }) {
  return (
    <label className={cn("block", className)}>
      <span className="mb-1.5 block text-sm font-medium">{label}</span>
      {children}
    </label>
  );
}


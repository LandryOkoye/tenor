// In-browser stand-in for the Rust API when VITE_MOCK=true.
import { ApiError } from "@/api/client";
import type * as T from "@/api/types";
import * as F from "./fixtures";

const wait = (ms = 250 + Math.random() * 250) => new Promise((r) => setTimeout(r, ms));
const inSecs = (s: number) => new Date(Date.now() + s * 1000).toISOString();
const notFound = () => new ApiError("NOT_FOUND", "Not found", 404);

const purchases = new Map<string, { view: T.BasketPurchaseView; submittedAt: number }>();
const quotes = new Map<string, T.BasketQuote>();
const orderQuotes = new Map<string, T.OrderQuote>();
const orders = new Map<string, { view: T.OrderView; submittedAt: number }>();

function summary(lines: string[], total: number, fee: number, panta: number, buy = true): T.TxSummary {
  return {
    summary_lines: lines,
    total_usdc: String(total),
    our_fee_usdc: String(fee),
    panta_fees_usdc: String(panta),
    notices: buy ? ["You cannot sell before these markets resolve. Positions are held until resolution, then claimed."] : [],
  };
}

export async function mockRequest<R>(method: string, fullPath: string, body?: unknown): Promise<R> {
  await wait();
  const [path, query = ""] = fullPath.split("?");
  const q = new URLSearchParams(query);
  const seg = path.split("/").filter(Boolean).map(decodeURIComponent); // ["v1", ...]
  const b = (body ?? {}) as Record<string, unknown>;
  const r = (v: unknown) => v as R;

  // auth
  if (path === "/v1/auth/nonce") return r({ nonce: "demo", message: "Sign in to Tenor (demo)", expires_at: inSecs(300) });
  if (path === "/v1/auth/verify") return r({ token: "demo-token", expires_at: inSecs(86_400) });

  // markets
  if (path === "/v1/categories") return r(F.categories);
  if (path === "/v1/markets") {
    let list = [...F.markets];
    const cat = q.get("category");
    const term = q.get("q")?.toLowerCase();
    if (cat) list = list.filter((m) => m.category?.toLowerCase() === cat.toLowerCase());
    if (term) list = list.filter((m) => m.title.toLowerCase().includes(term));
    const sort = q.get("sort") ?? "volume";
    if (sort === "volume") list.sort((a, b) => Number(b.volume_usdc) - Number(a.volume_usdc));
    if (sort === "closing_soon") list.sort((a, b) => +new Date(a.close_time!) - +new Date(b.close_time!));
    return r({
      items: list.map((m): T.MarketSummary => ({ id: m.id, title: m.title, category: m.category, phase: m.phase, status: m.status, close_time: m.close_time, resolve_time: m.resolve_time, yes_price: m.yes_price, no_price: m.no_price, volume_usdc: m.volume_usdc, image_url: m.image_url, stale: m.stale })),
    });
  }
  if (seg[1] === "markets" && seg[2]) {
    const m = F.markets.find((x) => x.id === seg[2]);
    if (seg[3] === "history") return r(F.history(seg[2], q.get("range") ?? "7d"));
    if (seg[3] === "trades") return r(F.trades(seg[2]));
    if (!m) throw notFound();
    return r(m);
  }

  // single orders
  if (path === "/v1/orders/quote") {
    const m = F.markets.find((x) => x.id === b.market_id)!;
    const amt = Number(b.amount_usdc);
    const price = Number(b.side === "yes" ? m.yes_price : m.no_price) * 1.01;
    const panta = Math.round(amt * 0.02);
    const quote: T.OrderQuote = { quote_id: crypto.randomUUID(), market_id: m.id, side: b.side as T.Side, amount_usdc: String(amt), expected_shares: ((amt - panta) / 1e6 / price).toFixed(2), avg_price: price.toFixed(3), panta_fee_usdc: String(panta), our_fee_usdc: "0", price_impact_bps: Math.round(amt / 1e6 / 4), expires_at: inSecs(90) };
    orderQuotes.set(quote.quote_id, quote);
    return r(quote);
  }
  if (seg[1] === "orders" && seg[3] === "build") {
    const qt = orderQuotes.get(seg[2]);
    if (!qt) throw new ApiError("QUOTE_STALE", "Quote expired", 409);
    const m = F.markets.find((x) => x.id === qt.market_id)!;
    const id = crypto.randomUUID();
    orders.set(id, { view: { order_id: id, market_id: m.id, market_title: m.title, side: qt.side, amount_usdc: qt.amount_usdc, expected_shares: qt.expected_shares, status: "built", tx_signature: null, error_code: null, created_at: inSecs(0), updated_at: inSecs(0) }, submittedAt: 0 });
    return r({ order_id: id, transactions: ["bW9jaw=="], summary: summary([`Buy ~${qt.expected_shares} ${qt.side.toUpperCase()} shares of "${m.title}" for ${(Number(qt.amount_usdc) / 1e6).toFixed(2)} USDC`], Number(qt.amount_usdc), 0, Number(qt.panta_fee_usdc)), expires_at: inSecs(60) });
  }
  if (seg[1] === "orders" && seg[2]) {
    const o = orders.get(seg[2]);
    if (!o) throw notFound();
    if (seg[3] === "submit") o.submittedAt = Date.now();
    const el = Date.now() - o.submittedAt;
    o.view = { ...o.view, status: el < 1500 ? "submitted" : el < 3000 ? "confirmed" : "filled", tx_signature: "5mockSig" };
    return r(o.view);
  }

  // baskets
  if (path === "/v1/baskets") return r(F.summaries());
  if (seg[1] === "baskets" && seg[2] === "quotes" && seg[4] === "build") {
    const qt = quotes.get(seg[3]);
    if (!qt) throw new ApiError("QUOTE_STALE", "Quote expired", 409);
    const t = F.baskets.find((x) => x.id === qt.template_id)!;
    const id = crypto.randomUUID();
    purchases.set(id, {
      submittedAt: 0,
      view: {
        id, template_id: t.id, template_slug: t.slug, template_title: t.title, amount_usdc: qt.amount_usdc, fee_usdc: qt.our_fee_usdc, status: "built", created_at: inSecs(0), updated_at: inSecs(0),
        legs: qt.legs.map((l, i) => ({ order_id: `${id}-${i}`, market_id: l.market_id, market_title: l.market_title, side: l.side, amount_usdc: l.allocation_usdc, shares: null, status: "pending", tx_signature: null, error_code: null, current_price: l.avg_price, claimable: false })),
      },
    });
    const lines = [`Buy ${qt.legs.length} positions for ${(Number(qt.amount_usdc) / 1e6).toFixed(2)} USDC in one transaction:`, ...qt.legs.map((l) => `${l.side.toUpperCase()} · ${l.market_title}: ${(Number(l.allocation_usdc) / 1e6).toFixed(2)} USDC → ~${l.expected_shares} shares`)];
    return r({ purchase_id: id, transactions: ["bW9jaw=="], summary: summary(lines, Number(qt.amount_usdc), Number(qt.our_fee_usdc), Number(qt.total_panta_fees_usdc)), expires_at: inSecs(60) });
  }
  if (seg[1] === "baskets" && seg[2] === "purchases" && seg[3]) {
    const p = purchases.get(seg[3]);
    if (!p) throw notFound();
    if (seg[4] === "submit") p.submittedAt = Date.now();
    const el = Date.now() - p.submittedAt;
    const status: T.PurchaseStatus = el < 1200 ? "submitted" : el < 2600 ? "confirmed" : "filled";
    p.view = {
      ...p.view,
      status,
      legs: p.view.legs.map((l, i) => ({ ...l, status: status === "filled" || el > 1600 + i * 500 ? "filled" : "pending", shares: status === "filled" ? (Number(l.amount_usdc) / 1e6 / Number(l.current_price)).toFixed(2) : null, tx_signature: status === "submitted" ? null : "5mockSig" })),
    };
    return r(p.view);
  }
  if (seg[1] === "baskets" && seg[3] === "quote") {
    const t = F.baskets.find((x) => x.id === seg[2]);
    if (!t) throw notFound();
    const amt = Number(b.amount_usdc);
    const fee = Math.round(amt * 0.0075);
    const net = amt - fee;
    const legs: T.BasketQuoteLeg[] = t.legs.map((l) => {
      const alloc = Math.floor((net * l.weight_bps) / 10_000);
      const panta = Math.round(alloc * 0.02);
      const price = Number(l.current_price) * 1.012;
      return { market_id: l.market_id, market_title: l.market_title, side: l.side, weight_bps: l.weight_bps, allocation_usdc: String(alloc), expected_shares: ((alloc - panta) / 1e6 / price).toFixed(2), avg_price: price.toFixed(3), panta_fee_usdc: String(panta), price_impact_bps: Math.max(4, Math.round(alloc / 1e6 / 2)) };
    });
    const quote: T.BasketQuote = { quote_id: crypto.randomUUID(), template_id: t.id, amount_usdc: String(amt), our_fee_usdc: String(fee), total_panta_fees_usdc: String(legs.reduce((s, l) => s + Number(l.panta_fee_usdc), 0)), legs, expires_at: inSecs(80) };
    quotes.set(quote.quote_id, quote);
    return r(quote);
  }
  if (seg[1] === "baskets" && seg[2]) {
    const t = F.baskets.find((x) => x.slug === seg[2]);
    if (!t) throw notFound();
    return r(t);
  }

  // portfolio + claims
  if (path === "/v1/portfolio") return r(F.portfolio);
  if (path === "/v1/portfolio/activity") return r(F.activity);
  if (path === "/v1/claims/build") {
    const ids = (b.market_ids as string[]) ?? [];
    const total = ids.length * 9_260_000;
    return r({ claim_id: crypto.randomUUID(), market_ids: ids, remaining_market_ids: [], transactions: ["bW9jaw=="], summary: summary([`Claim winnings on ${ids.length} market${ids.length === 1 ? "" : "s"}`], total, 0, 0, false), total_claimable_usdc: String(total) });
  }
  if (path === "/v1/claims/submit") return r({ claim_id: b.claim_id, signatures: ["5mockSig"], status: "confirmed" });

  // resolution
  if (path === "/v1/resolution/markets") return r(F.resolutionRows[(q.get("status") as T.ResolutionStatus) ?? "resolving_soon"] ?? []);
  if (seg[1] === "resolution" && seg[2] === "markets" && seg[3]) {
    const all = Object.values(F.resolutionRows).flat();
    const row = all.find((x) => x.market_id === seg[3]);
    const m = F.markets.find((x) => x.id === seg[3]);
    if (!row && !m) throw notFound();
    const title = row?.title ?? m!.title;
    const creator = row?.creator ?? m?.creator ?? null;
    const resolved = !!row?.resolved_at;
    const t0 = Date.now() - 40 * 86_400_000;
    const ev = (d: number, event_type: string, label: string, source: T.DataSource = "panta"): T.TimelineEvent => ({ ts: new Date(t0 + d * 86_400_000).toISOString(), event_type, label, explorer_url: "https://solscan.io/", source });
    const timeline = [ev(0, "created", "Market created by creator wallet"), ev(0.1, "trading_started", "Trading opened on the bonding curve")];
    if (row?.close_time) timeline.push(ev((+new Date(row.close_time) - t0) / 86_400_000, "closed", "Trading closed"));
    if (resolved) {
      timeline.push(ev((+new Date(row!.resolved_at!) - t0) / 86_400_000, "resolved", `Resolved ${row!.outcome?.toUpperCase()}`));
      timeline.push(ev((+new Date(row!.resolved_at!) - t0) / 86_400_000 + 0.01, "claims_open", "Claims opened for winning shares"));
    }
    return r({
      market_id: seg[3], title, status: row?.status ?? "open", rules_text: m?.rules_text ?? F.markets[0].rules_text, oracle: m?.oracle ?? "Official announcement", creator,
      expected_resolve_time: row?.expected_resolve_time ?? m?.resolve_time ?? null, resolved_at: row?.resolved_at ?? null, outcome: row?.outcome ?? null,
      last_price_before_resolution: row?.last_price_before_resolution ?? null, timeline, creator_scorecard: creator ? F.scorecard(creator) : null,
    } satisfies T.ResolutionMarketDetail);
  }
  if (seg[1] === "resolution" && seg[2] === "creators" && seg[3]) {
    return r({ scorecard: F.scorecard(seg[3]), markets: Object.values(F.resolutionRows).flat().filter((x) => x.creator === seg[3]) });
  }

  // admin
  if (path === "/v1/admin/panta-markets") {
    const term = q.get("q")?.toLowerCase() ?? "";
    return r(F.markets.filter((m) => m.title.toLowerCase().includes(term)).map((m) => ({ id: m.id, title: m.title, category: m.category, status: m.status, close_time: m.close_time, volume_usdc: m.volume_usdc, yes_price: m.yes_price, no_price: m.no_price })));
  }
  if (seg[1] === "admin" && seg[2] === "baskets") {
    const req = b as unknown as T.CreateBasketRequest;
    const status: T.BasketStatus = seg[4] === "publish" ? "published" : seg[4] === "close" ? "closed" : "draft";
    return r({ id: seg[3] ?? crypto.randomUUID(), slug: req.slug ?? "draft", title: req.title ?? "Draft", description: req.description ?? "", category: req.category ?? null, cover_url: null, status, min_buy_usdc: req.min_buy_usdc ?? "5000000", max_buy_usdc: req.max_buy_usdc ?? "500000000", legs: req.legs ?? [], warnings: ["Demo: liquidity checks are simulated"], created_at: inSecs(0), published_at: status === "published" ? inSecs(0) : null, closed_at: null } satisfies T.AdminBasketView);
  }

  throw new ApiError("NOT_FOUND", `Mock has no route for ${method} ${path}`, 404);
}

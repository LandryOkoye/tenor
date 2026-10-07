// Demo fixtures for VITE_MOCK=true only. Never shown as live: the app renders a
// "Demo data" banner whenever mock mode is on (brief §4.4).
import type * as T from "@/api/types";

const day = 86_400_000;
const now = Date.now();
const iso = (offsetMs: number) => new Date(now + offsetMs).toISOString();

type M = T.MarketDetail;
const mk = (id: string, title: string, category: string, yes: number, volume: number, closeDays: number, extra: Partial<M> = {}): M => ({
  id,
  title,
  category,
  phase: "trading",
  status: "open",
  close_time: iso(closeDays * day),
  resolve_time: iso((closeDays + 1) * day),
  yes_price: yes.toFixed(3),
  no_price: (1 - yes).toFixed(3),
  volume_usdc: String(volume * 1_000_000),
  image_url: null,
  rules_text:
    "Resolves YES if the outcome described in the title is confirmed by the official source listed below before the resolution time. Resolves NO otherwise. If the event is cancelled or postponed beyond the resolution time, the market resolves NO.",
  creator: "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  oracle: "Official source named in the market rules",
  stale: false,
  in_baskets: [],
  ...extra,
});

export const markets: M[] = [
  mk("mkt-btc-150k", "BTC above $150K before year end", "Crypto", 0.24, 210_900, 81),
  mk("mkt-sol-300", "SOL above $300 on 31 Dec", "Crypto", 0.38, 120_400, 81),
  mk("mkt-eth-etf", "ETH ETF net inflows top $1B in November", "Crypto", 0.33, 41_200, 50),
  mk("mkt-fed-dec", "Fed cuts rates at the December FOMC", "Economy", 0.29, 98_620, 60),
  mk("mkt-us-cpi", "US CPI below 2.5% in the next print", "Economy", 0.36, 32_050, 33),
  mk("mkt-us-unemp", "US unemployment rate at 4.5% or higher in December", "Economy", 0.41, 18_300, 65),
  mk("mkt-ucl-rm", "Real Madrid wins the Champions League", "Sports", 0.31, 77_455, 230),
  mk("mkt-lakers", "Lakers make the NBA playoffs", "Sports", 0.57, 26_910, 180),
  mk("mkt-ng-group-a", "Nigeria tops AFCON Group A", "Sports", 0.62, 48_210, 21),
  mk("mkt-ng-5goals", "Nigeria scores 5+ group-stage goals", "Sports", 0.41, 22_904, 21),
  mk("mkt-group-a-draw", "Group A has a 0–0 draw", "Sports", 0.33, 9_870, 21),
  mk("mkt-ai-model", "A frontier AI lab releases a new flagship model before year end", "Tech", 0.72, 54_300, 81),
  mk("mkt-foldable", "Apple announces a foldable iPhone in 2027", "Tech", 0.44, 21_800, 440),
  mk("mkt-film-2b", "A 2026 film grosses $2B+ worldwide", "Culture", 0.18, 15_780, 81),
  mk("mkt-grammy-afro", "An Afrobeats artist wins a major Grammy in 2027", "Culture", 0.44, 12_340, 120),
  mk("mkt-fr-turnout", "French 2027 first-round turnout above 75%", "Politics", 0.46, 19_900, 560),
  mk("mkt-uk-election", "UK general election called before 2028", "Politics", 0.21, 23_480, 440),
];

export const categories: T.Category[] = ["Sports", "Politics", "Economy", "Crypto", "Tech", "Culture"].map((n) => ({ id: n, name: n }));

type B = T.BasketTemplateDetail & { closes_in_seconds: number; leg_count: number };
const legs = (spec: Array<[string, T.Side, number]>): T.BasketLegView[] =>
  spec.map(([id, side, w]) => {
    const m = markets.find((x) => x.id === id)!;
    return { market_id: id, market_title: m.title, side, weight_bps: w, current_price: side === "yes" ? m.yes_price : m.no_price, close_time: m.close_time, tradable: true };
  });
const idx = (l: T.BasketLegView[]) => (l.reduce((s, x) => s + Number(x.current_price) * x.weight_bps, 0) / 10_000).toFixed(3);
const basket = (id: string, slug: string, title: string, category: string, description: string, l: T.BasketLegView[], bought: number, closeDays: number): B => ({
  id,
  slug,
  title,
  description,
  category,
  cover_url: null,
  status: "published",
  min_buy_usdc: "5000000",
  max_buy_usdc: "500000000",
  index_price: idx(l),
  legs: l,
  leg_count: l.length,
  total_bought_usdc: String(bought * 1_000_000),
  published_at: iso(-6 * day),
  closes_in_seconds: closeDays * 86400,
});

export const baskets: B[] = [
  basket(
    "b1000000-0000-4000-8000-000000000001",
    "fed-easing-cycle",
    "Fed easing cycle",
    "Economy",
    "A view that US inflation cools and the labour market softens enough for the Fed to cut in December.",
    legs([["mkt-fed-dec", "yes", 4000], ["mkt-us-cpi", "yes", 3000], ["mkt-us-unemp", "yes", 3000]]),
    31_310,
    33,
  ),
  basket(
    "b1000000-0000-4000-8000-000000000002",
    "crypto-year-end-rally",
    "Crypto year-end rally",
    "Crypto",
    "Risk-on into December: majors finish the year strong and ETF demand keeps building.",
    legs([["mkt-btc-150k", "yes", 4000], ["mkt-sol-300", "yes", 3000], ["mkt-eth-etf", "yes", 3000]]),
    27_880,
    50,
  ),
  basket(
    "b1000000-0000-4000-8000-000000000003",
    "afcon-group-a-favourites",
    "AFCON Group A favourites",
    "Sports",
    "The Super Eagles have the deepest squad in the group. This basket backs Nigeria to top it and score freely, with a hedge against a cagey 0–0 elsewhere.",
    legs([["mkt-ng-group-a", "yes", 4000], ["mkt-ng-5goals", "yes", 3000], ["mkt-group-a-draw", "no", 3000]]),
    18_420,
    21,
  ),
  basket(
    "b1000000-0000-4000-8000-000000000004",
    "global-pop-moments",
    "Global pop moments",
    "Culture",
    "Culture goes global: a record-breaking box office year and an Afrobeats win on music's biggest night.",
    legs([["mkt-film-2b", "yes", 5000], ["mkt-grammy-afro", "yes", 5000]]),
    4_120,
    81,
  ),
];

for (const b of baskets) for (const l of b.legs) markets.find((m) => m.id === l.market_id)!.in_baskets.push({ slug: b.slug, title: b.title });

export function summaries(): T.BasketTemplateSummary[] {
  return baskets.map((b) => ({
    id: b.id,
    slug: b.slug,
    title: b.title,
    description: b.description,
    category: b.category,
    cover_url: null,
    status: b.status,
    leg_count: b.leg_count,
    min_buy_usdc: b.min_buy_usdc,
    max_buy_usdc: b.max_buy_usdc,
    index_price: b.index_price,
    closes_in_seconds: b.closes_in_seconds as unknown as bigint,
    total_bought_usdc: b.total_bought_usdc,
    published_at: b.published_at,
  }));
}

export function history(id: string, range: string): T.MarketHistory {
  const m = markets.find((x) => x.id === id);
  const end = Number(m?.yes_price ?? 0.5);
  const n = range === "1d" ? 48 : range === "7d" ? 84 : 120;
  const span = range === "1d" ? day : range === "7d" ? 7 * day : 30 * day;
  let seed = id.split("").reduce((s, c) => s + c.charCodeAt(0), 0);
  const rand = () => ((seed = (seed * 9301 + 49297) % 233280) / 233280 - 0.5);
  const pts: number[] = [end];
  for (let i = 1; i < n; i++) pts.push(Math.min(0.97, Math.max(0.03, pts[i - 1] + rand() * 0.045)));
  pts.reverse();
  return {
    market_id: id,
    range,
    points: pts.map((p, i) => ({ ts: new Date(now - span + (span / (n - 1)) * i).toISOString(), yes_price: p.toFixed(3), no_price: (1 - p).toFixed(3) })),
  };
}

export function trades(id: string): T.MarketTrade[] {
  const m = markets.find((x) => x.id === id);
  const y = Number(m?.yes_price ?? 0.5);
  return Array.from({ length: 14 }, (_, i) => {
    const side: T.Side = i % 3 === 1 ? "no" : "yes";
    const price = side === "yes" ? y - i * 0.002 : 1 - y + i * 0.001;
    const amt = [12, 50, 5, 120, 20, 8, 250, 15, 40, 10, 75, 6, 30, 18][i];
    return { signature: `5tX${i}q9Lm${id.length}Rz${i * 7}Ka8Wn3pYc2`, side, amount_usdc: String(amt * 1e6), shares: (amt / price).toFixed(2), price: price.toFixed(3), ts: iso(-(i * 23 + 4) * 60_000) };
  });
}

export const portfolio: T.Portfolio = {
  wallet: "DemoTenor1111111111111111111111111111111111",
  total_value_usdc: "71840000",
  total_cost_basis_usdc: "65000000",
  total_unrealized_pnl_usdc: "6840000",
  single_positions: [
    { market_id: "mkt-sol-300", market_title: "SOL above $300 on 31 Dec", side: "yes", shares: "52.63", current_price: "0.380", value_usdc: "20000000", cost_basis_usdc: "17500000", unrealized_pnl_usdc: "2500000", resolved: false, won: null, claimable: false },
    { market_id: "mkt-eurovision", market_title: "Eurovision 2026 winner is from the Nordics", side: "no", shares: "14.20", current_price: null, value_usdc: "14200000", cost_basis_usdc: "8000000", unrealized_pnl_usdc: "6200000", resolved: true, won: true, claimable: true },
    { market_id: "mkt-us-open", market_title: "A first-time champion wins the US Open (men)", side: "yes", shares: "9.00", current_price: null, value_usdc: "0", cost_basis_usdc: "5000000", unrealized_pnl_usdc: "-5000000", resolved: true, won: false, claimable: false },
  ],
  baskets: [
    {
      purchase_id: "p1000000-0000-4000-8000-000000000001",
      template_slug: "crypto-year-end-rally",
      template_title: "Crypto year-end rally",
      status: "active",
      cost_basis_usdc: "25000000",
      current_value_usdc: "27640000",
      unrealized_pnl_usdc: "2640000",
      legs_resolved: 0,
      legs_total: 3,
      modified_outside: false,
      legs: [
        { order_id: "o1", market_id: "mkt-btc-150k", market_title: "BTC above $150K before year end", side: "yes", shares: "44.54", status: "filled", value_usdc: "10690000", claimable: false },
        { order_id: "o2", market_id: "mkt-sol-300", market_title: "SOL above $300 on 31 Dec", side: "yes", shares: "20.97", status: "filled", value_usdc: "7970000", claimable: false },
        { order_id: "o3", market_id: "mkt-eth-etf", market_title: "ETH ETF net inflows top $1B in November", side: "yes", shares: "27.21", status: "filled", value_usdc: "8980000", claimable: false },
      ],
    },
    {
      purchase_id: "p1000000-0000-4000-8000-000000000002",
      template_slug: "global-pop-moments",
      template_title: "Global pop moments",
      status: "partially_resolved",
      cost_basis_usdc: "10000000",
      current_value_usdc: "9200000",
      unrealized_pnl_usdc: "-800000",
      legs_resolved: 1,
      legs_total: 2,
      modified_outside: false,
      legs: [
        { order_id: "o4", market_id: "mkt-grammy-afro", market_title: "An Afrobeats artist wins a major Grammy in 2027", side: "yes", shares: "11.10", status: "filled", value_usdc: "4880000", claimable: false },
        { order_id: "o5", market_id: "mkt-box-office", market_title: "Summer box office tops $4B in the US", side: "yes", shares: "4.32", status: "resolved_win", value_usdc: "4320000", claimable: true },
      ],
    },
  ],
};

export const activity: T.ActivityItem[] = [
  { kind: "basket_buy", market_id: null, market_title: "Crypto year-end rally", basket_purchase_id: "p1000000-0000-4000-8000-000000000001", side: null, amount_usdc: "25000000", tx_signature: "4Hc9...mock", ts: iso(-2 * day) },
  { kind: "buy", market_id: "mkt-sol-300", market_title: "SOL above $300 on 31 Dec", basket_purchase_id: null, side: "yes", amount_usdc: "17500000", tx_signature: "3kLp...mock", ts: iso(-5 * day) },
  { kind: "claim", market_id: "mkt-wc-qual", market_title: "Morocco qualifies for the 2026 World Cup knockout stage", basket_purchase_id: null, side: "yes", amount_usdc: "12000000", tx_signature: "2zQe...mock", ts: iso(-12 * day) },
];

export const resolutionRows: Record<T.ResolutionStatus, T.ResolutionMarketRow[]> = {
  open: [],
  resolving_soon: markets.slice(0, 5).map((m, i) => ({ market_id: m.id, title: m.title, status: "resolving_soon", close_time: iso((i + 1) * 0.4 * day), expected_resolve_time: iso((i + 1.5) * 0.4 * day), resolved_at: null, outcome: null, last_price_before_resolution: null, creator: m.creator, source: "panta" })),
  resolved: [
    { market_id: "mkt-eurovision", title: "Eurovision 2026 winner is from the Nordics", status: "resolved", close_time: iso(-9 * day), expected_resolve_time: iso(-8 * day), resolved_at: iso(-8 * day + 3_600_000), outcome: "no", last_price_before_resolution: "0.71", creator: "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU", source: "panta" },
    { market_id: "mkt-us-open", title: "A first-time champion wins the US Open (men)", status: "resolved", close_time: iso(-20 * day), expected_resolve_time: iso(-19 * day), resolved_at: iso(-19 * day + 7_200_000), outcome: "no", last_price_before_resolution: "0.93", creator: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", source: "panta" },
    { market_id: "mkt-box-office", title: "Summer box office tops $4B in the US", status: "resolved", close_time: iso(-30 * day), expected_resolve_time: iso(-29 * day), resolved_at: iso(-29 * day), outcome: "yes", last_price_before_resolution: "0.64", creator: "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU", source: "panta" },
  ],
  delayed: [
    { market_id: "mkt-launch", title: "Starship completes an orbital flight by September", status: "delayed", close_time: iso(-12 * day), expected_resolve_time: iso(-10 * day), resolved_at: null, outcome: null, last_price_before_resolution: null, creator: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM", source: "app_analysis" },
  ],
};

export const scorecard = (creator: string): T.CreatorScorecard => ({
  creator,
  markets_created: 42 as unknown as bigint,
  markets_resolved: 37 as unknown as bigint,
  pct_resolved_on_time: "0.89",
  median_delay_hours: "2.5",
  surprise_resolutions: 1 as unknown as bigint,
  total_volume_usdc: "812400000000",
  source: "app_analysis",
});

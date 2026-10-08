import { Link } from "react-router";
import {
  ArrowRight,
  ArrowUpRight,
  Bank,
  Basket,
  ChartLineUp,
  CheckCircle,
  Cpu,
  CurrencyBtc,
  CurrencyCircleDollar,
  FilmSlate,
  FingerprintSimple,
  Globe,
  Hourglass,
  Lightning,
  LockSimple,
  Scales,
  Trophy,
} from "@phosphor-icons/react";
import { Logo, Mark } from "@/components/brand/Logo";
import { ButtonLink } from "@/components/ui/Button";
import { PoweredByPanta } from "@/components/ui/primitives";
import { SiteFooter } from "@/components/layout/SiteFooter";
import { useBaskets } from "@/api/queries";
import { formatPrice, formatUsdcCompact } from "@/lib/money";
import { shortDuration } from "@/lib/time";
import { MOCK } from "@/lib/env";
import { ConvergeArt, HeroPill, Phone, Reveal, Tile } from "./visuals";

export function LandingPage() {
  return (
    <div className="overflow-x-clip bg-paper">
      {MOCK && <div className="bg-accent px-4 py-1.5 text-center text-xs font-medium">Demo data: mock mode is on.</div>}
      <MarketingNav />
      <Hero />
      <Introducing />
      <Coverage />
      <OneApp />
      <FeatureRows />
      <ThemesRail />
      <DarkSection />
      <FinalCta />
      <SiteFooter big />
    </div>
  );
}

function MarketingNav() {
  return (
    <header className="relative z-30 mx-auto flex max-w-7xl items-center justify-between gap-4 px-4 py-5 sm:px-8">
      <Link to="/" className="text-[26px]" aria-label="Tenor home">
        <Logo />
      </Link>
      <nav aria-label="Primary" className="hidden md:block">
        <ul className="flex items-center gap-1 rounded-full border border-line bg-surface/80 p-1 text-sm shadow-soft backdrop-blur">
          {[
            ["Markets", "/markets"],
            ["Baskets", "/baskets"],
            ["Resolution", "/resolution"],
            ["Learn", "/learn"],
          ].map(([l, to]) => (
            <li key={to}>
              <Link to={to} className="block rounded-full px-4 py-2 font-medium text-ink/80 hover:bg-sunk hover:text-ink">
                {l}
              </Link>
            </li>
          ))}
        </ul>
      </nav>
      <ButtonLink to="/markets" size="sm" className="h-10">
        Open app <ArrowRight size={16} weight="bold" aria-hidden="true" />
      </ButtonLink>
    </header>
  );
}

/* ───────────────────────── Hero ───────────────────────── */

function Hero() {
  return (
    <section className="relative mx-auto max-w-7xl px-4 pb-20 pt-8 sm:px-8 sm:pb-24 sm:pt-16 lg:pb-36">
      <h1 className="display-xl relative text-[18vw] text-ink sm:text-[clamp(3.6rem,12.5vw,11.5rem)]">
        <span className="block">
          Trade{" "}
          <HeroPill className="ml-[0.08em]">
            <PillIcon>
              <ChartLineUp weight="bold" />
            </PillIcon>
            <Tile desktopOnly className="-top-[0.55em] left-[0.18em] size-[0.62em]" rotate={-8}>
              <ChartLineUp weight="bold" className="size-[58%] text-ink" />
            </Tile>
            <span
              className="absolute -bottom-[0.22em] -right-[0.18em] hidden whitespace-nowrap rounded-full bg-surface px-[0.9em] py-[0.35em] font-mono text-[0.1em] font-bold leading-none tracking-normal text-yes shadow-float animate-float sm:inline-block"
              style={{ ["--r" as string]: "8deg", animationDelay: "700ms" }}
              aria-hidden="true"
            >
              YES 62¢ ▲4
            </span>
          </HeroPill>
        </span>
        <span className="relative block sm:pl-[0.5em]">
          <span className="relative mr-[0.1em] hidden align-middle sm:inline-block">
            <span className="grid size-[0.5em] place-items-center rounded-[22%] bg-surface shadow-float animate-float" style={{ ["--r" as string]: "-6deg" }} aria-hidden="true">
              <CurrencyCircleDollar weight="fill" className="size-[60%] text-[#2775CA]" />
            </span>
          </span>
          the whole
        </span>
        <span className="block sm:pl-[2.2em]">
          <HeroPill className="mr-[0.12em] w-[1.5em]">
            <PillIcon>
              <Mark inverted className="h-[0.26em] w-auto" />
            </PillIcon>
            <Tile desktopOnly className="top-[0.03em] -left-[0.28em] size-[0.56em]" rotate={6} delay={300}>
              <Mark className="w-[64%]" />
            </Tile>
            <Tile desktopOnly className="-bottom-[0.45em] left-[0.95em] size-[0.48em]" rotate={-10} delay={1100}>
              <Globe weight="duotone" className="size-[58%] text-accent" />
            </Tile>
          </HeroPill>
          story<span className="text-accent">.</span>
        </span>
      </h1>

      {/* Phone: the floating tiles become a legible chip row. */}
      <ul className="mt-7 flex flex-wrap gap-2 sm:hidden" aria-label="Highlights">
        <li className="rounded-full bg-yes-soft px-3 py-1.5 font-mono text-xs font-bold text-yes">YES 62¢ ▲4</li>
        <li className="inline-flex items-center gap-1.5 rounded-full bg-surface px-3 py-1.5 text-xs font-medium shadow-soft">
          <span className="size-1.5 rounded-full bg-accent" aria-hidden="true" /> Live P&L
        </li>
        <li className="rounded-full bg-surface px-3 py-1.5 text-xs font-medium shadow-soft">Verifiable resolution</li>
      </ul>

      <div className="mt-8 grid gap-8 sm:mt-12 md:mt-16 md:grid-cols-[minmax(0,27rem)_1fr] md:items-end">
        <div className="animate-rise" style={{ animationDelay: "150ms" }}>
          <p className="text-[17px] leading-relaxed text-muted sm:text-lg">
            Tenor makes prediction markets feel like <b className="font-medium text-ink">investing, not gambling</b>. Take a position on real-world outcomes, track it like a portfolio, and see exactly how every market resolves.
          </p>
          <div className="mt-7 grid grid-cols-2 gap-2 sm:flex sm:flex-wrap sm:items-center sm:gap-3">
            <ButtonLink to="/markets" size="lg" className="bg-gradient-to-b from-ink-3 to-ink px-4 sm:px-7">
              Explore markets
            </ButtonLink>
            <ButtonLink to="/learn" size="lg" variant="secondary" className="px-4 sm:border-transparent sm:bg-transparent sm:px-7">
              How it works <ArrowRight size={18} className="hidden sm:block" aria-hidden="true" />
            </ButtonLink>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 md:justify-end">
          <PoweredByPanta />
          <span className="hidden text-xs text-faint sm:inline" aria-hidden="true">·</span>
          <span className="text-xs text-muted">USDC on Solana · non-custodial · worldwide</span>
        </div>
      </div>
    </section>
  );
}

/** Icon centred inside a hero pill; phones only (desktop uses floating tiles). */
function PillIcon({ children }: { children: React.ReactNode }) {
  return <span className="absolute inset-0 grid place-items-center text-paper [&>svg]:h-[0.3em] [&>svg]:w-auto sm:hidden">{children}</span>;
}

/* ───────────────────────── Introducing ───────────────────────── */

function Introducing() {
  return (
    <section className="relative mx-auto grid max-w-7xl gap-16 px-4 pb-28 sm:px-8 lg:grid-cols-[1.1fr_1fr] lg:pb-40">
      <div className="flex flex-col justify-between gap-16">
        <Reveal>
          <h2 className="max-w-[17ch] text-[clamp(2rem,4.4vw,3.6rem)] font-semibold leading-[1.05] tracking-[-0.04em]">
            A trading app for real-world outcomes. <span className="text-faint">Not a betting slip.</span>
          </h2>
        </Reveal>
        <Reveal delay={120}>
          <div className="max-w-sm">
            <p className="font-medium">Introducing Tenor</p>
            <p className="mt-1.5 text-muted">
              Prices read as probabilities. Positions sit in a portfolio with cost basis and live P&L. Every market shows its rules, sources and resolution timeline. And it all settles in USDC, straight from your own wallet.
            </p>
            <Link to="/learn" className="mt-4 inline-flex items-center gap-1.5 text-sm font-medium underline-offset-4 hover:underline">
              How prediction markets work <ArrowUpRight size={14} aria-hidden="true" />
            </Link>
          </div>
        </Reveal>
      </div>
      <Reveal delay={200} className="relative flex justify-center lg:justify-end">
        <div className="absolute -right-24 top-10 size-[420px] rounded-full bg-accent/15 blur-3xl" aria-hidden="true" />
        <Phone className="rotate-[4deg]">
          <div className="px-5 pt-6">
            <p className="label-mono text-muted">Portfolio</p>
            <div className="mt-3 rounded-2xl bg-ink p-4 text-paper">
              <p className="text-[11px] text-paper/60">Total value</p>
              <p className="font-display text-[34px] font-semibold tracking-tight num">$1,284.50</p>
              <p className="text-xs text-[#5fd6a4] num">+$96.20 · +8.1% unrealised</p>
              <svg viewBox="0 0 200 40" className="mt-3 h-10 w-full" aria-hidden="true">
                <path d="M0 32 L20 30 L40 33 L60 26 L80 27 L100 20 L120 22 L140 15 L160 17 L180 9 L200 6" fill="none" stroke="var(--color-accent)" strokeWidth="2.5" strokeLinejoin="round" />
              </svg>
            </div>
            <ul className="mt-4 space-y-2">
              {[
                ["BTC above $150K by year end", "YES", "24¢", "+12%"],
                ["Fed cuts rates in December", "NO", "71¢", "+5%"],
                ["Real Madrid wins the UCL", "YES", "31¢", "−3%"],
              ].map(([t, s, p, d]) => (
                <li key={t} className="flex items-center gap-3 rounded-xl bg-surface px-3 py-2.5 shadow-soft">
                  <span className={`rounded-full px-1.5 py-0.5 font-mono text-[9px] font-bold ${s === "YES" ? "bg-yes-soft text-yes" : "bg-no-soft text-no"}`}>{s}</span>
                  <span className="flex-1 text-[12px] font-medium leading-tight">{t}</span>
                  <span className="text-right">
                    <span className="block text-[12px] font-semibold num">{p}</span>
                    <span className={`block text-[10px] num ${d.startsWith("−") ? "text-no" : "text-yes"}`}>{d}</span>
                  </span>
                </li>
              ))}
            </ul>
            <div className="mt-4 grid h-11 place-items-center rounded-full bg-ink text-[13px] font-medium text-paper">Claim winnings · $42.00</div>
          </div>
        </Phone>
        <div className="absolute bottom-16 left-0 hidden rounded-2xl bg-surface p-3.5 shadow-float sm:block lg:-left-6 animate-float" style={{ ["--r" as string]: "-3deg" }} aria-hidden="true">
          <p className="label-mono text-muted">Market resolved</p>
          <p className="mt-1 flex items-center gap-1.5 text-sm font-medium">
            <CheckCircle weight="fill" className="text-yes" size={18} /> ETH ETF inflows top $1B
          </p>
          <p className="mt-0.5 text-xs text-yes num">+$42.00 claimable</p>
        </div>
      </Reveal>
    </section>
  );
}

/* ───────────────────────── Coverage ───────────────────────── */

function Coverage() {
  const items = [
    { icon: Trophy, label: "Sports" },
    { icon: Bank, label: "Politics" },
    { icon: ChartLineUp, label: "Economy & rates" },
    { icon: CurrencyBtc, label: "Crypto" },
    { icon: Cpu, label: "Tech & AI" },
    { icon: FilmSlate, label: "Culture" },
  ];
  return (
    <section className="mx-auto max-w-7xl px-4 pb-28 sm:px-8 lg:pb-36">
      <p className="font-medium">Markets on what moves the world</p>
      <p className="text-muted">Live markets from Panta on Solana, settled in USDC. Open worldwide, wherever prediction markets are legal.</p>
      <ul className="mt-10 grid grid-cols-2 gap-x-6 gap-y-8 sm:grid-cols-3 lg:grid-cols-6">
        {items.map(({ icon: I, label }) => (
          <li key={label} className="flex items-center gap-2.5 text-ink/70">
            <I size={24} weight="duotone" aria-hidden="true" />
            <span className="font-display text-lg font-semibold tracking-tight">{label}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

/* ───────────────────────── One app ───────────────────────── */

function OneApp() {
  return (
    <section className="relative mx-auto max-w-7xl px-4 pb-28 sm:px-8 lg:pb-40">
      <p aria-hidden="true" className="display-xl pointer-events-none absolute inset-x-0 top-24 hidden select-none text-center text-[clamp(5rem,15vw,15rem)] leading-[0.85] text-ink/[0.025] md:block">
        TRADE THE
        <br />
        OUTCOME
      </p>
      <div className="relative max-w-md">
        <p className="font-medium">Everything a trader expects.</p>
        <p className="mt-1 text-muted">Clear prices, a real portfolio and claims that land in your wallet. Plus baskets when you want to back a whole theme.</p>
      </div>
      <div className="relative mt-14 grid gap-5 md:grid-cols-3">
        <Reveal>
          <FeatureCard title="Trade any market" body="Prices shown as cents and chance, with the rules, a price chart and the live trade tape on every market.">
            <div className="rounded-xl bg-surface p-4 shadow-soft">
              <p className="text-sm font-medium">Fed cuts rates in December</p>
              <div className="mt-3 grid grid-cols-2 gap-2">
                <span className="rounded-lg bg-yes-soft px-3 py-2.5 text-center text-sm font-semibold text-yes num">Yes 29¢</span>
                <span className="rounded-lg bg-no-soft px-3 py-2.5 text-center text-sm font-semibold text-no num">No 71¢</span>
              </div>
              <div className="mt-3 flex h-1.5 overflow-hidden rounded-full bg-sunk">
                <span className="w-[29%] bg-yes" />
              </div>
              <p className="mt-2 text-xs text-muted num">29% chance · $18.6K volume</p>
            </div>
          </FeatureCard>
        </Reveal>
        <Reveal delay={100}>
          <FeatureCard title="Track it like a portfolio" body="Cost basis, current value and unrealised P&L across every position, with winnings ready to claim.">
            <div className="space-y-2">
              {[
                ["SOL above $300 on 31 Dec", "+$2.50", true],
                ["US CPI below 2.5%", "+$6.20", true],
                ["Lakers make the playoffs", "−$1.10", false],
              ].map(([t, v, up]) => (
                <div key={t as string} className="flex items-center gap-3 rounded-xl bg-surface px-3.5 py-3 shadow-soft">
                  <span className={`h-8 w-1.5 rounded-full ${up ? "bg-yes" : "bg-no"}`} />
                  <span className="flex-1 text-sm font-medium">{t as string}</span>
                  <span className={`text-sm font-semibold num ${up ? "text-yes" : "text-no"}`}>{v as string}</span>
                </div>
              ))}
            </div>
          </FeatureCard>
        </Reveal>
        <Reveal delay={200}>
          <FeatureCard title="Or back a whole theme" body="Baskets bundle 2–4 related markets you buy in one signature. Each leg pays on its own, so partly right still pays.">
            <div className="rounded-xl bg-surface p-4 shadow-soft">
              <p className="flex items-center gap-2 text-sm font-medium">
                <Basket size={16} aria-hidden="true" /> Crypto year-end rally
              </p>
              <div className="mt-3 flex h-2 gap-1 overflow-hidden rounded-full">
                <span className="w-[40%] rounded-full bg-accent" />
                <span className="w-[30%] rounded-full bg-ink" />
                <span className="w-[30%] rounded-full bg-ink/40" />
              </div>
              <p className="mt-2 flex justify-between text-xs text-muted num">
                <span>3 legs · 1 signature</span>
                <span>Index 31¢</span>
              </p>
            </div>
          </FeatureCard>
        </Reveal>
      </div>
    </section>
  );
}

function FeatureCard({ title, body, children }: { title: string; body: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="flex h-72 flex-col justify-center rounded-[var(--radius-card)] bg-sunk p-6">{children}</div>
      <h3 className="mt-5 px-1 text-lg font-semibold tracking-tight">{title}</h3>
      <p className="mt-1 px-1 text-muted">{body}</p>
    </div>
  );
}

/* ───────────────────────── Feature rows ───────────────────────── */

function FeatureRows() {
  return (
    <section className="mx-4 rounded-[2.5rem] bg-sunk/70 px-3 py-6 sm:mx-6 sm:px-6 sm:py-10">
      <div className="mx-auto max-w-6xl space-y-6 sm:space-y-10">
        <Row
          title="Priced like probabilities."
          lead="Read a market in one glance"
          body="A YES price of 62¢ means the market sees roughly a 62% chance. Buy at 62¢, and each share pays $1.00 if it happens."
          stat="62¢"
          statLabel="≈ 62% chance, pays $1.00"
          art={
            <svg viewBox="0 0 400 300" className="absolute inset-0 h-full w-full" preserveAspectRatio="none" aria-hidden="true">
              <defs>
                <linearGradient id="rowArea" x1="0" x2="0" y1="0" y2="1">
                  <stop offset="0" stopColor="#ff6b1a" stopOpacity="0.35" />
                  <stop offset="1" stopColor="#ff6b1a" stopOpacity="0" />
                </linearGradient>
              </defs>
              <path d="M0 220 L40 210 L80 228 L120 190 L160 196 L200 160 L240 170 L280 130 L320 138 L360 108 L400 96 L400 300 L0 300Z" fill="url(#rowArea)" />
              <path d="M0 220 L40 210 L80 228 L120 190 L160 196 L200 160 L240 170 L280 130 L320 138 L360 108 L400 96" fill="none" stroke="rgb(250 250 248 / 0.9)" strokeWidth="2.5" vectorEffect="non-scaling-stroke" />
            </svg>
          }
        />
        <Row
          title="Track it like a portfolio."
          lead="Cost basis, value and P&L in one place"
          body="Every position and basket in your wallet, valued at live prices. When a market resolves your way, claim it in one tap."
          stat="P&L"
          statLabel="live across every position"
          art={
            <div className="absolute left-6 top-8 space-y-2.5 sm:left-10 sm:top-10">
              {[
                ["BTC above $150K", "+12.4%", true],
                ["Fed cuts in December", "+5.1%", true],
                ["Real Madrid wins the UCL", "−3.2%", false],
              ].map(([t, v, up]) => (
                <div key={t as string} className="flex w-64 items-center gap-2.5 rounded-xl bg-paper/10 px-3 py-2.5 text-sm backdrop-blur-md">
                  <span className="flex-1">{t as string}</span>
                  <span className={`num ${up ? "text-[#5fd6a4]" : "text-[#ff8a7a]"}`}>{v as string}</span>
                </div>
              ))}
            </div>
          }
        />
        <Row
          title="Know how it resolves."
          lead="Rules, sources and a timeline for every market"
          body="See who created a market, what decides it and when it should resolve, plus each creator's on-time record."
          stat="Every"
          statLabel="market gets a resolution timeline"
          art={
            <ol className="absolute left-6 top-8 space-y-3 sm:left-10 sm:top-10">
              {["Market created", "Trading closed", "Outcome proposed", "Resolved YES · claims open"].map((t, i) => (
                <li key={t} className="flex items-center gap-3 text-sm">
                  <span className={`size-3 rounded-full ${i === 3 ? "bg-accent" : "bg-paper/70"}`} />
                  <span className={i === 3 ? "font-medium" : "text-paper/75"}>{t}</span>
                </li>
              ))}
            </ol>
          }
        />
      </div>
    </section>
  );
}

function Row({ title, lead, body, stat, statLabel, art }: { title: string; lead: string; body: string; stat: string; statLabel: string; art: React.ReactNode }) {
  return (
    <Reveal>
      <article className="grid overflow-hidden rounded-[2rem] bg-surface md:grid-cols-2">
        <div className="p-7 sm:p-10">
          <h3 className="max-w-[14ch] text-[clamp(1.8rem,3.2vw,2.6rem)] font-semibold leading-[1.08] tracking-[-0.035em]">{title}</h3>
          <p className="mt-6 font-medium">{lead}</p>
          <p className="mt-1 max-w-sm text-muted">{body}</p>
        </div>
        <div className="relative min-h-[300px] overflow-hidden bg-ink text-paper md:rounded-l-[2rem]">
          {art}
          <div className="absolute bottom-6 right-6 text-right sm:bottom-8 sm:right-8">
            <p className="font-display text-[clamp(3.5rem,7vw,5.5rem)] font-medium leading-none tracking-[-0.05em]">{stat}</p>
            <p className="mt-1 text-sm font-medium">{statLabel}</p>
          </div>
        </div>
      </article>
    </Reveal>
  );
}

/* ───────────────────────── Baskets rail (one feature, not the headline) ───────────────────────── */

const EXAMPLES = [
  { title: "Fed easing cycle", category: "Economy", legs: 3, price: "0.34" },
  { title: "Crypto year-end rally", category: "Crypto", legs: 3, price: "0.31" },
  { title: "AFCON Group A favourites", category: "Sports", legs: 3, price: "0.52" },
  { title: "Global pop moments", category: "Culture", legs: 2, price: "0.48" },
];

function ThemesRail() {
  const { data } = useBaskets();
  const live = data && data.length > 0;
  const cards = live
    ? data.map((b) => ({ key: b.slug, to: `/baskets/${b.slug}`, title: b.title, category: b.category ?? "Basket", legs: b.leg_count, price: b.index_price, closes: b.closes_in_seconds == null ? null : Number(b.closes_in_seconds), bought: b.total_bought_usdc }))
    : EXAMPLES.map((e) => ({ key: e.title, to: "/baskets", title: e.title, category: e.category, legs: e.legs, price: e.price, closes: null, bought: null }));

  return (
    <section className="py-28 lg:py-40">
      <div className="mx-auto flex max-w-7xl flex-wrap items-end justify-between gap-6 px-4 sm:px-8">
        <div className="max-w-xl">
          <p className="flex items-center gap-2 text-muted">
            <Basket size={18} aria-hidden="true" /> Baskets
          </p>
          <h2 className="mt-2 text-[clamp(2.4rem,6vw,4.75rem)] font-semibold leading-none tracking-[-0.045em]">Have a thesis? Buy it whole.</h2>
          <p className="mt-4 text-muted">Curated bundles of related markets, bought in one signature. Diversified like an index, and each leg pays out on its own.</p>
          {!live && <p className="mt-3 label-mono text-faint">Examples · live baskets appear here once published</p>}
        </div>
        <ButtonLink to="/baskets" variant="secondary">
          All baskets <ArrowRight size={16} aria-hidden="true" />
        </ButtonLink>
      </div>
      <ul className="no-scrollbar mt-12 flex snap-x snap-mandatory gap-4 overflow-x-auto px-4 pb-4 sm:px-8 xl:px-[max(2rem,calc((100vw-80rem)/2+2rem))]">
        {cards.map((c, i) => (
          <li key={c.key} className="snap-start">
            <Link to={c.to} className="group relative block h-[420px] w-[78vw] max-w-[320px] overflow-hidden rounded-[1.75rem] bg-ink text-paper sm:w-[300px]">
              <ConvergeArt legs={c.legs} className="absolute inset-0 h-full w-full opacity-90 transition-transform duration-700 group-hover:scale-105" animated={false} />
              <div className={`absolute inset-0 ${["bg-gradient-to-b from-[#0e3b2c]/70", "bg-gradient-to-b from-[#3b230e]/70", "bg-gradient-to-b from-[#1c1d3b]/70", "bg-gradient-to-b from-[#3b0e2a]/60", "bg-gradient-to-b from-[#0e2a3b]/70"][i % 5]} to-transparent`} />
              <span className="absolute left-4 top-4 rounded-full bg-paper/15 px-3 py-1.5 text-xs font-medium backdrop-blur">{c.category}</span>
              <div className="absolute inset-x-0 bottom-0 p-5">
                <p className="text-xs text-paper/60">Index price</p>
                <p className="font-display text-[56px] font-semibold leading-none tracking-[-0.05em] num">
                  {formatPrice(c.price).cents}
                  <sup className="ml-1 align-top text-xs font-medium tracking-normal text-paper/60">{c.legs} legs</sup>
                </p>
                <div className="mt-4 flex items-center justify-between rounded-2xl bg-paper/10 px-4 py-3 backdrop-blur-md">
                  <span className="text-[15px] font-medium leading-tight">{c.title}</span>
                  <ArrowUpRight size={18} className="shrink-0 transition-transform group-hover:-translate-y-0.5 group-hover:translate-x-0.5" aria-hidden="true" />
                </div>
                {(c.closes != null || c.bought) && (
                  <p className="mt-2 flex justify-between px-1 text-[11px] text-paper/55 num">
                    <span>{c.closes != null ? `Closes in ${shortDuration(c.closes)}` : ""}</span>
                    <span>{c.bought ? `${formatUsdcCompact(c.bought)} bought` : ""}</span>
                  </p>
                )}
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}

/* ───────────────────────── Dark section ───────────────────────── */

function DarkSection() {
  return (
    <section className="mx-2 rounded-[2.5rem] bg-ink px-4 py-24 text-paper sm:mx-4 sm:px-8 lg:py-32">
      <div className="mx-auto max-w-5xl">
        <Reveal className="text-center">
          <h2 className="text-[clamp(2.4rem,6vw,4.5rem)] font-semibold tracking-[-0.045em]">And that's not all.</h2>
          <p className="mx-auto mt-3 max-w-md text-paper/60">Built for the questions traders actually ask: will this resolve fairly, and is my money safe?</p>
        </Reveal>

        <div className="mt-16 grid gap-4 md:grid-cols-2">
          <Reveal className="md:col-span-2">
            <div className="grid overflow-hidden rounded-[1.75rem] bg-ink-2 md:grid-cols-[1.2fr_1fr]">
              <p className="p-7 text-[clamp(1.5rem,2.6vw,2.1rem)] font-semibold leading-snug tracking-[-0.03em] sm:p-10">
                <span className="text-paper/45">Contested resolutions shouldn't be a mystery.</span> The Resolution Center shows the rules, the sources and every step to the final outcome.
              </p>
              <div className="relative min-h-[260px] p-7 sm:p-10">
                <div className="rounded-2xl bg-paper p-5 text-ink shadow-float">
                  <div className="flex items-center justify-between">
                    <p className="label-mono text-muted">Resolution</p>
                    <span className="rounded-full bg-yes-soft px-2 py-0.5 text-[11px] font-semibold text-yes">On time</span>
                  </div>
                  <p className="mt-2 font-medium leading-snug">ETH ETF net inflows top $1B in November</p>
                  <ol className="mt-4 space-y-3 border-l border-line pl-4">
                    {[
                      ["Trading closed", "30 Nov, 23:59"],
                      ["Resolved YES", "1 Dec, 14:05"],
                      ["Claims opened", "1 Dec, 14:06"],
                    ].map(([t, d], i) => (
                      <li key={t} className="relative text-sm">
                        <span className={`absolute -left-[21px] top-1.5 size-2.5 rounded-full ${i === 1 ? "bg-accent" : "bg-ink"}`} />
                        <span className="font-medium">{t}</span> <span className="text-muted num">· {d}</span>
                      </li>
                    ))}
                  </ol>
                </div>
              </div>
            </div>
          </Reveal>
          <Reveal>
            <div className="h-full rounded-[1.75rem] bg-ink-2 p-7 sm:p-9">
              <p className="font-medium">Creator track records</p>
              <p className="mt-1 text-paper/55">Anyone can create a market on Panta. See how often each creator resolves on time, before you buy.</p>
              <div className="mt-8 grid grid-cols-3 gap-3">
                {[
                  ["89%", "on time"],
                  ["2.5h", "median delay"],
                  ["37", "resolved"],
                ].map(([v, l]) => (
                  <div key={l} className="rounded-2xl bg-paper/[0.06] p-4">
                    <p className="font-display text-2xl font-semibold tracking-tight num">{v}</p>
                    <p className="text-xs text-paper/55">{l}</p>
                  </div>
                ))}
              </div>
              <p className="mt-4 label-mono text-paper/40">Tenor analysis · labelled separately from Panta data</p>
            </div>
          </Reveal>
          <Reveal delay={100}>
            <div className="h-full rounded-[1.75rem] bg-ink-2 p-7 sm:p-9">
              <p className="font-medium">Honest about the trade-offs</p>
              <p className="mt-1 text-paper/55">Panta markets are buy-only today. We tell you before every signature, not after.</p>
              <ul className="mt-8 space-y-3 text-sm">
                {[
                  [Hourglass, "Positions are held until the market resolves"],
                  [Scales, "Every fee is shown line by line"],
                  [Lightning, "Stale prices are labelled, never hidden"],
                ].map(([I, t]) => {
                  const Icon = I as typeof Hourglass;
                  return (
                    <li key={t as string} className="flex items-center gap-3 rounded-xl bg-paper/[0.06] px-4 py-3">
                      <Icon size={18} className="text-accent" aria-hidden="true" /> {t as string}
                    </li>
                  );
                })}
              </ul>
            </div>
          </Reveal>
        </div>

        <div className="mt-28 grid items-center gap-16 md:grid-cols-2">
          <Reveal>
            <h2 className="text-[clamp(2.6rem,6vw,4.75rem)] font-semibold leading-[0.98] tracking-[-0.045em]">
              Your wallet.
              <br />
              Your positions.
            </h2>
            <p className="mt-5 max-w-md text-paper/60">Tenor is fully non-custodial. Your USDC and positions live in your own Solana wallet, and nothing moves without your signature.</p>
            <ul className="mt-12 space-y-8">
              {[
                [LockSimple, "We never hold your funds", "No deposits and no withdrawals. You trade straight from the wallet you already use."],
                [FingerprintSimple, "You sign every transaction", "Phantom, Solflare, Backpack and more. We never ask for a seed phrase."],
                [CheckCircle, "Simulated before you sign", "Every transaction is checked against an allowlist and simulated first. If it would fail, you never see it."],
              ].map(([I, t, d]) => {
                const Icon = I as typeof LockSimple;
                return (
                  <li key={t as string} className="flex gap-4">
                    <span className="grid size-11 shrink-0 place-items-center rounded-xl border border-paper/15 bg-paper/[0.04]">
                      <Icon size={20} aria-hidden="true" />
                    </span>
                    <span>
                      <span className="block font-medium">{t as string}</span>
                      <span className="mt-0.5 block max-w-xs text-sm text-paper/55">{d as string}</span>
                    </span>
                  </li>
                );
              })}
            </ul>
          </Reveal>
          <Reveal delay={150} className="flex justify-center">
            <Phone className="-rotate-[5deg] bg-ink-3">
              <div className="flex h-full flex-col px-5 pt-8 text-ink">
                <p className="text-center text-[13px] font-medium">Review and sign</p>
                <div className="mt-5 rounded-2xl bg-sunk p-4 text-[12px] leading-relaxed">
                  <p className="flex gap-2"><span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-accent" />Buy ~81 YES shares of "BTC above $150K by year end" for 20.00 USDC</p>
                </div>
                <dl className="mt-4 space-y-2 text-[12px]">
                  {[
                    ["Panta fee (est.)", "$0.40"],
                    ["Price impact", "0.6%"],
                  ].map(([k, v]) => (
                    <div key={k} className="flex justify-between"><dt className="text-muted">{k}</dt><dd className="num">{v}</dd></div>
                  ))}
                  <div className="flex justify-between border-t border-line pt-2 font-semibold"><dt>Total</dt><dd className="num">$20.00</dd></div>
                </dl>
                <p className="mt-4 rounded-xl bg-warn-soft px-3 py-2.5 text-[11px]">You cannot sell before this market resolves.</p>
                <div className="mt-auto mb-8 grid h-11 place-items-center rounded-full bg-ink text-[13px] font-medium text-paper">Sign in wallet</div>
              </div>
            </Phone>
          </Reveal>
        </div>
      </div>
    </section>
  );
}

/* ───────────────────────── Final CTA ───────────────────────── */

function FinalCta() {
  return (
    <section className="mx-auto max-w-6xl px-4 py-28 sm:px-8 lg:py-36">
      <Reveal>
        <div className="relative overflow-hidden rounded-[2rem] border border-line bg-surface px-5 py-10 shadow-soft sm:rounded-[2.5rem] sm:px-12 sm:py-16">
          <h2 className="display-xl text-[12.5vw] sm:text-[clamp(3rem,9vw,7.5rem)]">
            Have a view?
            <br />
            <HeroPill className="mr-[0.12em]">
              <PillIcon>
                <Mark inverted className="h-[0.26em] w-auto" />
              </PillIcon>
              <Tile desktopOnly className="-top-[0.35em] left-[0.35em] size-[0.55em]" rotate={-6}>
                <Mark className="w-[64%]" />
              </Tile>
            </HeroPill>
            Trade it<span className="text-accent">.</span>
          </h2>
          <div className="mt-10 flex flex-col gap-6 sm:flex-row sm:items-end sm:justify-between">
            <p className="max-w-sm text-lg text-muted">Pick a market, choose an amount, sign once. It takes about a minute with a funded Solana wallet.</p>
            <div className="flex flex-wrap gap-3">
              <ButtonLink to="/markets" size="lg">Explore markets</ButtonLink>
              <ButtonLink to="/learn#usdc" size="lg" variant="secondary">Get USDC on Solana</ButtonLink>
            </div>
          </div>
          <div className="pointer-events-none absolute -right-20 -top-20 size-72 rounded-full bg-accent/20 blur-3xl" aria-hidden="true" />
        </div>
      </Reveal>
    </section>
  );
}


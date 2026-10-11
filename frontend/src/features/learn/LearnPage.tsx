import { useEffect } from "react";
import { useLocation } from "react-router";
import { ArrowUpRight, CheckCircle, XCircle } from "@phosphor-icons/react";
import { ButtonLink } from "@/components/ui/Button";
import { Card } from "@/components/ui/primitives";
import { Mark } from "@/components/brand/Logo";

const SECTIONS = [
  { id: "markets", title: "What's a prediction market?" },
  { id: "prices", title: "Reading prices" },
  { id: "portfolio", title: "Your portfolio" },
  { id: "baskets", title: "Baskets" },
  { id: "held", title: "Held until resolution" },
  { id: "fees", title: "Fees" },
  { id: "wallet", title: "Get a wallet" },
  { id: "usdc", title: "Get USDC on Solana" },
  { id: "risks", title: "Risks" },
];

export default function LearnPage() {
  const { hash } = useLocation();
  useEffect(() => {
    if (hash) document.getElementById(hash.slice(1))?.scrollIntoView({ block: "start" });
  }, [hash]);

  return (
    <div className="grid gap-10 lg:grid-cols-[220px_1fr]">
      <nav aria-label="On this page" className="hidden lg:block">
        <ul className="sticky top-24 space-y-1 text-sm">
          {SECTIONS.map((s) => (
            <li key={s.id}>
              <a href={`#${s.id}`} className="block rounded-lg px-3 py-2 text-muted hover:bg-sunk hover:text-ink">{s.title}</a>
            </li>
          ))}
        </ul>
      </nav>
      <article className="max-w-2xl">
        <p className="label-mono text-muted">Learn</p>
        <h1 className="mt-2 text-4xl font-semibold leading-[1.02] tracking-[-0.045em] sm:text-5xl">Tenor in plain language.</h1>
        <p className="mt-4 text-lg text-muted">Prediction markets work like trading, not betting: prices move with new information and positions have a value you can track. Five minutes to see how it works, and what can go wrong.</p>

        <Section id="markets" title="What's a prediction market?">
          <p>A market asks a yes-or-no question about a real event, like "Will BTC close the year above $150K?" or "Will the Fed cut rates in December?". You buy YES or NO shares. When the event is decided, every winning share pays <b>$1.00</b> in USDC and losing shares pay nothing.</p>
          <p>Between now and then, the price moves as news comes in, just like any traded asset. That's what makes it a market rather than a bet: you're pricing information.</p>
        </Section>

        <Section id="prices" title="Reading prices">
          <p>Prices run from 0¢ to 100¢. A YES price of <b>62¢</b> roughly means the crowd sees a <b>62% chance</b>. Buy at 62¢ and you get $1.00 back if YES wins, so a $0.38 profit per share.</p>
          <p>Markets on Panta use a bonding curve: each buy nudges the price up. Big buys in thin markets get a worse average price (the "price impact" shown in every quote). So a price isn't a pure probability.</p>
        </Section>

        <Section id="portfolio" title="Your portfolio">
          <p>Every position you hold is valued at the current market price, so you always see your <b>cost basis</b>, <b>current value</b> and <b>unrealised P&L</b>, like a brokerage account. When a market resolves your way, claim your winnings from the Portfolio page, one at a time or all at once.</p>
        </Section>

        <Section id="baskets" title="Baskets">
          <p>Have a thesis that spans several events? A basket bundles 2–4 related markets, each with a side and a weight, and you buy all of them in one signature. Think of it as a small index for a view, unlike a parlay:</p>
          <div className="grid gap-3 sm:grid-cols-2">
            <Card className="p-5">
              <p className="font-semibold">Parlay</p>
              <ul className="mt-3 space-y-2 text-sm">
                <li className="flex gap-2"><XCircle size={18} className="shrink-0 text-no" aria-hidden="true" /> Every leg must win</li>
                <li className="flex gap-2"><XCircle size={18} className="shrink-0 text-no" aria-hidden="true" /> 2 of 3 right pays $0</li>
              </ul>
            </Card>
            <Card className="border-ink p-5">
              <p className="flex items-center gap-2 font-semibold"><Mark className="h-4" /> Tenor basket</p>
              <ul className="mt-3 space-y-2 text-sm">
                <li className="flex gap-2"><CheckCircle size={18} weight="fill" className="shrink-0 text-yes" aria-hidden="true" /> Each leg pays on its own</li>
                <li className="flex gap-2"><CheckCircle size={18} weight="fill" className="shrink-0 text-yes" aria-hidden="true" /> 2 of 3 right still pays</li>
              </ul>
            </Card>
          </div>
          <p>Example: $30 across three equal legs. Two win at an average 50¢ buy price: each $10 leg bought ~20 shares, so you claim ~$40. The losing leg pays $0.</p>
        </Section>

        <Section id="held" title="Held until resolution">
          <p>Panta markets are <b>buy-only</b>: there's no way to sell a position before the market resolves. Once it resolves, you claim winnings to your wallet from your Portfolio. Only put in what you're happy to leave until then.</p>
        </Section>

        <Section id="fees" title="Fees">
          <ul className="list-disc space-y-1 pl-5">
            <li><b>Panta protocol fee</b>, often 2%, set per market on-chain.</li>
            <li><b>Tenor fee</b> on basket purchases, a small percentage added as a plain USDC transfer inside the same transaction you sign.</li>
            <li><b>Solana network fee</b>, usually a fraction of a cent, paid in SOL.</li>
          </ul>
          <p>Every fee is listed line by line before you sign. Nothing is taken afterwards.</p>
        </Section>

        <Section id="wallet" title="Get a wallet">
          <p>Tenor never holds your money. You need a Solana wallet app or browser extension:</p>
          <div className="flex flex-wrap gap-2">
            {[["Phantom", "https://phantom.com/download"], ["Solflare", "https://solflare.com/download"], ["Backpack", "https://backpack.app/downloads"]].map(([n, u]) => (
              <a key={n} href={u} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 rounded-full border border-line bg-surface px-4 py-2 text-sm font-medium hover:border-ink">{n} <ArrowUpRight size={14} aria-hidden="true" /></a>
            ))}
          </div>
          <p>Write your recovery phrase down offline. Nobody from Tenor will ever ask for it.</p>
        </Section>

        <Section id="usdc" title="Get USDC on Solana">
          <ol className="list-decimal space-y-1 pl-5">
            <li>Buy USDC on an exchange that supports Solana withdrawals.</li>
            <li>Withdraw to your wallet address and pick the <b>Solana</b> network. Sending on another network can lose funds.</li>
            <li>Keep a little SOL (around $1) for network fees.</li>
          </ol>
          <p className="text-sm text-muted">Local-currency on-ramps are planned.</p>
        </Section>

        <Section id="risks" title="Risks">
          <ul className="list-disc space-y-1 pl-5">
            <li>You can lose everything you put into a market or basket.</li>
            <li>Resolutions are decided on Panta. Tenor shows how, but can't change outcomes.</li>
            <li>Anyone can create a Panta market. Check the creator's track record in the Resolution Center.</li>
            <li>Prediction markets are restricted in some countries. You're responsible for using them legally. 18+ only.</li>
          </ul>
        </Section>

        <div className="mt-14 flex flex-wrap gap-3">
          <ButtonLink to="/baskets">Explore baskets</ButtonLink>
          <ButtonLink to="/legal" variant="secondary">Terms & risk disclosure</ButtonLink>
        </div>
      </article>
    </div>
  );
}

function Section({ id, title, children }: { id: string; title: string; children: React.ReactNode }) {
  return (
    <section id={id} className="mt-14 scroll-mt-24">
      <h2 className="text-2xl font-semibold tracking-[-0.03em]">{title}</h2>
      <div className="mt-4 space-y-4 leading-relaxed text-ink/85">{children}</div>
    </section>
  );
}

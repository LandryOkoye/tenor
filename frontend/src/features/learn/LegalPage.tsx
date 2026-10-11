import { Badge } from "@/components/ui/primitives";

/** Placeholder legal copy (brief §14). Must be replaced by counsel-reviewed text before real users deposit. */
export default function LegalPage() {
  return (
    <article className="max-w-2xl">
      <Badge tone="warn">Draft · pending legal review</Badge>
      <h1 className="mt-4 text-4xl font-semibold tracking-[-0.04em]">Terms, privacy & risk</h1>

      <section id="risk" className="mt-10 scroll-mt-24 space-y-3 leading-relaxed text-ink/85">
        <h2 className="text-2xl font-semibold tracking-tight text-ink">Risk disclosure</h2>
        <p>Trading prediction markets can lose you all the money you put in. Positions can't be sold before a market resolves. Prices come from bonding curves on the Panta protocol and aren't guarantees of probability.</p>
        <p>Resolutions are made on Panta. Tenor shows information about how markets resolve but can't change, delay or overturn an outcome. Information marked "Tenor analysis" is our own computation and may be incomplete.</p>
        <p>Transactions on Solana are final. Check every transaction summary before you sign.</p>
      </section>

      <section id="terms" className="mt-10 scroll-mt-24 space-y-3 leading-relaxed text-ink/85">
        <h2 className="text-2xl font-semibold tracking-tight text-ink">Terms of service</h2>
        <p>You must be 18 or older and permitted to use prediction markets where you live. Tenor isn't available in restricted countries. Tenor is a non-custodial interface: we never hold your funds or keys, and every transaction is signed by you in your own wallet.</p>
      </section>

      <section id="privacy" className="mt-10 scroll-mt-24 space-y-3 leading-relaxed text-ink/85">
        <h2 className="text-2xl font-semibold tracking-tight text-ink">Privacy</h2>
        <p>We store your public wallet address and the trades you make through Tenor to show your portfolio. We never ask for or store private keys or recovery phrases.</p>
      </section>
    </article>
  );
}

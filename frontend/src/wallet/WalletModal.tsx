import { useWallet } from "@solana/wallet-adapter-react";
import { WalletReadyState } from "@solana/wallet-adapter-base";
import { ArrowUpRight, Flask, Wallet } from "@phosphor-icons/react";
import { Dialog } from "@/components/ui/Dialog";
import { Link } from "react-router";

const GET_WALLETS = [
  { name: "Phantom", url: "https://phantom.com/download" },
  { name: "Solflare", url: "https://solflare.com/download" },
  { name: "Backpack", url: "https://backpack.app/downloads" },
];

export function WalletModal({ open, onClose, onDemo }: { open: boolean; onClose: () => void; onDemo?: () => void }) {
  const { wallets, select } = useWallet();
  const detected = wallets.filter((w) => w.readyState === WalletReadyState.Installed || w.readyState === WalletReadyState.Loadable);

  return (
    <Dialog open={open} onClose={onClose} title="Connect a wallet">
      <p className="-mt-2 mb-5 text-sm text-muted">Tenor is non-custodial. Your USDC and positions stay in your own Solana wallet, and you approve every transaction.</p>
      <ul className="space-y-2">
        {detected.map((w) => (
          <li key={w.adapter.name}>
            <button
              onClick={() => {
                select(w.adapter.name);
                onClose();
              }}
              className="flex w-full items-center gap-3 rounded-2xl border border-line bg-surface px-4 py-3.5 text-left transition-colors hover:border-ink"
            >
              <img src={w.adapter.icon} alt="" className="size-8 rounded-lg" width={32} height={32} />
              <span className="flex-1 font-medium">{w.adapter.name}</span>
              <span className="label-mono text-yes">Detected</span>
            </button>
          </li>
        ))}
        {onDemo && (
          <li>
            <button onClick={onDemo} className="flex w-full items-center gap-3 rounded-2xl border border-dashed border-line-strong px-4 py-3.5 text-left hover:border-ink">
              <span className="grid size-8 place-items-center rounded-lg bg-accent-soft text-accent-ink">
                <Flask size={18} aria-hidden="true" />
              </span>
              <span className="flex-1">
                <span className="block font-medium">Demo wallet</span>
                <span className="block text-xs text-muted">Mock mode only. Nothing is signed or sent.</span>
              </span>
            </button>
          </li>
        )}
      </ul>

      {detected.length === 0 && (
        <div className="mt-2 rounded-2xl bg-sunk p-4">
          <p className="flex items-center gap-2 font-medium">
            <Wallet size={18} aria-hidden="true" /> No Solana wallet found
          </p>
          <p className="mt-1 text-sm text-muted">Install one of these, then refresh this page.</p>
          <div className="mt-3 flex flex-wrap gap-2">
            {GET_WALLETS.map((w) => (
              <a key={w.name} href={w.url} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 rounded-full bg-surface px-3.5 py-2 text-sm font-medium border border-line hover:border-ink">
                {w.name} <ArrowUpRight size={14} aria-hidden="true" />
              </a>
            ))}
          </div>
        </div>
      )}
      <p className="mt-5 text-xs text-muted">
        New to wallets?{" "}
        <Link to="/learn#wallet" onClick={onClose} className="font-medium text-ink underline underline-offset-4">
          Read the 2-minute guide
        </Link>
        . We never ask for your seed phrase.
      </p>
    </Dialog>
  );
}

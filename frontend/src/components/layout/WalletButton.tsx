import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { CaretDown, Copy, SignOut, ShieldCheck, Wallet } from "@phosphor-icons/react";
import { Button } from "@/components/ui/Button";
import { shortAddress, useTenorWallet } from "@/wallet/useTenorWallet";
import { useSession } from "@/wallet/session";
import { friendlyError } from "@/api/client";

export function WalletButton() {
  const w = useTenorWallet();
  const { session, ensureSession, signOut, signingIn } = useSession();
  const [open, setOpen] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  if (!w.address) {
    return (
      <Button size="sm" onClick={w.openModal} loading={w.connecting} className="h-10">
        <Wallet size={18} weight="bold" aria-hidden="true" /> Connect
      </Button>
    );
  }

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        aria-haspopup="menu"
        className="flex h-10 items-center gap-2 rounded-full border border-line bg-surface pl-1.5 pr-3 text-sm font-medium hover:border-line-strong"
      >
        {w.walletIcon ? <img src={w.walletIcon} alt="" className="size-7 rounded-full" /> : <span className="grid size-7 place-items-center rounded-full bg-accent-soft text-accent-ink"><Wallet size={15} aria-hidden="true" /></span>}
        <span className="num">{shortAddress(w.address)}</span>
        {!session && <span className="size-2 rounded-full bg-accent" aria-label="Not signed in" />}
        <CaretDown size={14} aria-hidden="true" />
      </button>
      {open && (
        <div role="menu" className="absolute right-0 mt-2 w-64 rounded-2xl border border-line bg-surface p-2 shadow-float animate-rise">
          <div className="px-3 py-2">
            <p className="text-xs text-muted">{w.walletName}</p>
            <p className="font-mono text-xs break-all">{w.address}</p>
          </div>
          {!session && (
            <button
              role="menuitem"
              onClick={async () => {
                setErr(null);
                try {
                  await ensureSession();
                  setOpen(false);
                } catch (e) {
                  setErr(friendlyError(e));
                }
              }}
              className="flex w-full items-center gap-2.5 rounded-xl bg-accent-soft px-3 py-2.5 text-left text-sm font-medium"
            >
              <ShieldCheck size={18} aria-hidden="true" /> {signingIn ? "Check your wallet…" : "Sign in to trade"}
            </button>
          )}
          {err && <p className="px-3 py-1 text-xs text-no">{err}</p>}
          <Link role="menuitem" to="/portfolio" onClick={() => setOpen(false)} className="flex items-center gap-2.5 rounded-xl px-3 py-2.5 text-sm hover:bg-sunk">
            <Wallet size={18} aria-hidden="true" /> Portfolio
          </Link>
          <button role="menuitem" onClick={() => navigator.clipboard?.writeText(w.address!)} className="flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-sm hover:bg-sunk">
            <Copy size={18} aria-hidden="true" /> Copy address
          </button>
          <div className="my-1 border-t border-line" />
          <button
            role="menuitem"
            onClick={async () => {
              signOut();
              await w.disconnect();
              setOpen(false);
            }}
            className="flex w-full items-center gap-2.5 rounded-xl px-3 py-2.5 text-left text-sm text-no hover:bg-no-soft"
          >
            <SignOut size={18} aria-hidden="true" /> Disconnect
          </button>
        </div>
      )}
    </div>
  );
}

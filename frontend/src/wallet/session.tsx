import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import bs58 from "bs58";
import { api } from "@/api/endpoints";
import { setAuthToken, setUnauthorizedHandler } from "@/api/client";
import { readStore, writeStore } from "@/lib/storage";
import { useTenorWallet } from "./useTenorWallet";

type Session = { wallet: string; token: string; expires_at: string };
type Ctx = {
  session: Session | null;
  signingIn: boolean;
  /** Returns a valid session, running Sign-In-With-Solana if needed. */
  ensureSession: () => Promise<Session>;
  signOut: () => void;
};

const KEY = "tenor.session";
const SessionContext = createContext<Ctx | null>(null);

function valid(s: Session | null, wallet: string | null): s is Session {
  return !!s && !!wallet && s.wallet === wallet && new Date(s.expires_at).getTime() - Date.now() > 30_000;
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const wallet = useTenorWallet();
  const [session, setSession] = useState<Session | null>(() => readStore<Session | null>(KEY, null));
  const [signingIn, setSigningIn] = useState(false);

  const current = valid(session, wallet.address) ? session : null;

  useEffect(() => {
    setAuthToken(current?.token ?? null);
  }, [current?.token]);

  const signOut = useCallback(() => {
    setSession(null);
    writeStore(KEY, null);
    setAuthToken(null);
  }, []);

  useEffect(() => {
    setUnauthorizedHandler(signOut);
    return () => setUnauthorizedHandler(null);
  }, [signOut]);

  const ensureSession = useCallback(async () => {
    if (current) return current;
    if (!wallet.address) {
      wallet.openModal();
      throw new Error("Connect a wallet to continue.");
    }
    if (!wallet.signMessage) throw new Error("This wallet can't sign messages, so it can't sign in. Try Phantom, Solflare or Backpack.");
    setSigningIn(true);
    try {
      const { message } = await api.nonce(wallet.address);
      const sig = await wallet.signMessage(new TextEncoder().encode(message));
      const res = await api.verify({ wallet: wallet.address, signature: bs58.encode(sig) });
      const s: Session = { wallet: wallet.address, token: res.token, expires_at: res.expires_at };
      setAuthToken(s.token);
      setSession(s);
      writeStore(KEY, s);
      return s;
    } finally {
      setSigningIn(false);
    }
  }, [current, wallet]);

  const value = useMemo(() => ({ session: current, signingIn, ensureSession, signOut }), [current, signingIn, ensureSession, signOut]);
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): Ctx {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error("useSession must be used inside SessionProvider");
  return ctx;
}

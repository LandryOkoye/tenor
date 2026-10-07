import { createContext, useContext } from "react";

/** One wallet surface for the app: real wallet-adapter, or the demo wallet in mock mode. */
export type TenorWallet = {
  address: string | null;
  connected: boolean;
  connecting: boolean;
  walletName: string | null;
  walletIcon: string | null;
  isDemo: boolean;
  signMessage: ((msg: Uint8Array) => Promise<Uint8Array>) | null;
  /** Signs base64 unsigned VersionedTransactions, returns base64 signed ones in the same order. */
  signTransactions: (b64: string[]) => Promise<string[]>;
  disconnect: () => Promise<void>;
  openModal: () => void;
};

export const TenorWalletContext = createContext<TenorWallet | null>(null);

export function useTenorWallet(): TenorWallet {
  const ctx = useContext(TenorWalletContext);
  if (!ctx) throw new Error("useTenorWallet must be used inside WalletProviders");
  return ctx;
}

export function shortAddress(a: string | null | undefined, n = 4): string {
  if (!a) return "";
  return a.length <= n * 2 + 1 ? a : `${a.slice(0, n)}…${a.slice(-n)}`;
}

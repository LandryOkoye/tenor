import { useCallback, useMemo, useState, type ReactNode } from "react";
import { WalletProvider, useWallet } from "@solana/wallet-adapter-react";
import { MOCK } from "@/lib/env";
import { TenorWalletContext, type TenorWallet } from "./useTenorWallet";
import { SessionProvider } from "./session";
import { signWithAdapter } from "./sign";
import { WalletModal } from "./WalletModal";

const DEMO_ADDRESS = "DemoTenor1111111111111111111111111111111111";

function Bridge({ children }: { children: ReactNode }) {
  const adapter = useWallet();
  const [open, setOpen] = useState(false);
  const [demo, setDemo] = useState(false);

  const signTransactions = useCallback(
    async (b64: string[]) => {
      if (demo) {
        await new Promise((r) => setTimeout(r, 700));
        return b64;
      }
      return signWithAdapter(adapter, b64);
    },
    [adapter, demo],
  );

  const value = useMemo<TenorWallet>(() => {
    if (demo) {
      return {
        address: DEMO_ADDRESS,
        connected: true,
        connecting: false,
        walletName: "Demo wallet",
        walletIcon: null,
        isDemo: true,
        signMessage: async () => new Uint8Array(64),
        signTransactions,
        disconnect: async () => setDemo(false),
        openModal: () => setOpen(true),
      };
    }
    return {
      address: adapter.publicKey?.toBase58() ?? null,
      connected: adapter.connected,
      connecting: adapter.connecting,
      walletName: adapter.wallet?.adapter.name ?? null,
      walletIcon: adapter.wallet?.adapter.icon ?? null,
      isDemo: false,
      signMessage: adapter.signMessage ?? null,
      signTransactions,
      disconnect: () => adapter.disconnect(),
      openModal: () => setOpen(true),
    };
  }, [adapter, demo, signTransactions]);

  return (
    <TenorWalletContext.Provider value={value}>
      <SessionProvider>{children}</SessionProvider>
      <WalletModal open={open} onClose={() => setOpen(false)} onDemo={MOCK ? () => { setDemo(true); setOpen(false); } : undefined} />
    </TenorWalletContext.Provider>
  );
}

export function WalletProviders({ children }: { children: ReactNode }) {
  // Wallet Standard auto-detects Phantom, Solflare, Backpack, etc.; no adapters bundled.
  return (
    <WalletProvider wallets={[]} autoConnect>
      <Bridge>{children}</Bridge>
    </WalletProvider>
  );
}

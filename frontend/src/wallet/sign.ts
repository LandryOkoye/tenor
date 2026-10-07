import type { WalletContextState } from "@solana/wallet-adapter-react";

/**
 * Deserialize the backend's unsigned v0 transactions, have the wallet sign them
 * (signTransaction, NOT signAndSend: our backend broadcasts, brief §7.3) and re-encode.
 */
export async function signWithAdapter(wallet: WalletContextState, b64: string[]): Promise<string[]> {
  const { VersionedTransaction } = await import("@solana/web3.js");
  const txs = b64.map((s) => VersionedTransaction.deserialize(Uint8Array.from(atob(s), (c) => c.charCodeAt(0))));
  let signed: InstanceType<typeof VersionedTransaction>[];
  if (txs.length > 1 && wallet.signAllTransactions) signed = await wallet.signAllTransactions(txs);
  else if (wallet.signTransaction) {
    signed = [];
    for (const tx of txs) signed.push(await wallet.signTransaction(tx));
  } else throw new Error("This wallet can't sign transactions. Try Phantom, Solflare or Backpack.");
  return signed.map((tx) => {
    const bytes = tx.serialize();
    let bin = "";
    for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
    return btoa(bin);
  });
}

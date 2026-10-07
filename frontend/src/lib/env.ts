export const API_BASE = (import.meta.env.VITE_API_BASE_URL as string | undefined)?.replace(/\/$/, "") ?? "http://localhost:8080";
export const RPC_URL = (import.meta.env.VITE_SOLANA_RPC_URL as string | undefined) ?? "https://api.mainnet-beta.solana.com";
export const MOCK = import.meta.env.VITE_MOCK === "true";
export const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
export const PANTA_URL = "https://panta.market";

import { API_BASE, MOCK } from "@/lib/env";
import type { ApiError as ApiErrorEnvelope } from "./types";

/** Error carrying the backend's `{ error: { code, message } }` envelope (brief §9). */
export class ApiError extends Error {
  code: string;
  status: number;
  details?: unknown;
  constructor(code: string, message: string, status: number, details?: unknown) {
    super(message);
    this.code = code;
    this.status = status;
    this.details = details;
  }
}

let token: string | null = null;
let onUnauthorized: (() => void) | null = null;

export function setAuthToken(t: string | null) {
  token = t;
}
export function setUnauthorizedHandler(fn: (() => void) | null) {
  onUnauthorized = fn;
}

type Method = "GET" | "POST" | "PATCH" | "DELETE";

export async function request<T>(method: Method, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  if (MOCK) {
    const { mockRequest } = await import("@/mock/handler");
    return mockRequest<T>(method, path, body);
  }
  let res: Response;
  try {
    res = await fetch(API_BASE + path, {
      method,
      signal,
      headers: {
        Accept: "application/json",
        ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  } catch (e) {
    if ((e as Error).name === "AbortError") throw e;
    throw new ApiError("NETWORK", "Can't reach Tenor right now. Check your connection and try again.", 0);
  }
  if (res.status === 401) onUnauthorized?.();
  const text = await res.text();
  const json = text ? safeJson(text) : null;
  if (!res.ok) {
    const env = json as ApiErrorEnvelope | null;
    throw new ApiError(
      env?.error?.code ?? `HTTP_${res.status}`,
      env?.error?.message ?? `Request failed (${res.status})`,
      res.status,
      env?.error?.details,
    );
  }
  return json as T;
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** Plain-language copy for codes the user can act on. */
export function friendlyError(e: unknown): string {
  if (e instanceof ApiError) {
    switch (e.code) {
      case "QUOTE_STALE":
      case "QUOTE_EXPIRED":
        return "Prices moved since your quote. Get a fresh quote to continue.";
      case "TX_EXPIRED":
        return "The transaction expired before it landed, so nothing was bought. Get a fresh quote and try again.";
      case "TX_VALIDATION_FAILED":
        return "The signed transaction didn't match what Tenor built, so we didn't send it. Please try again.";
      case "DUPLICATE_TX":
      case "ALREADY_SUBMITTED":
        return "This transaction was already submitted. Check your portfolio for its status.";
      case "RATE_LIMITED":
        return "Too many requests. Wait a few seconds and try again.";
      case "PANTA_UNAVAILABLE":
        return "Panta, our market data and settlement provider, isn't responding. Try again shortly.";
      case "AMOUNT_OUT_OF_RANGE":
        return "That amount is outside the allowed range for this trade.";
      case "INVALID_SLIPPAGE":
        return "Slippage must be between 0.01% and 10%.";
      case "VAULT_NOT_LIVE":
        return "This basket token isn't open for deposits right now.";
      case "VAULT_NOT_SETTLED":
        return "Basket tokens can be redeemed once every market in the basket has resolved.";
      case "VAULT_DISABLED":
        return "Tokenized baskets aren't available right now.";
      case "SLIPPAGE_EXCEEDED":
        return "Prices moved more than your slippage tolerance. Requote to see the new total.";
      case "LEG_NOT_TRADABLE":
        return "One of these markets is no longer open for trading.";
      case "SIMULATION_FAILED":
        return "This transaction failed a safety simulation, so we didn't send it to your wallet. Check your USDC balance and try again.";
      case "TX_TOO_LARGE":
        return "This basket is too large for one transaction right now.";
      case "UNAUTHORIZED":
      case "HTTP_401":
        return "Your session expired. Sign in with your wallet again.";
      default:
        return e.message;
    }
  }
  if (e instanceof Error) {
    if (/user rejected|rejected the request|declined/i.test(e.message)) return "You cancelled the request in your wallet.";
    return e.message;
  }
  return "Something went wrong.";
}

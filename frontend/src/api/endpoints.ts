import { request } from "./client";
import type * as T from "./types";

const qs = (params: Record<string, string | number | null | undefined>) => {
  const s = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) if (v !== null && v !== undefined && v !== "") s.set(k, String(v));
  const out = s.toString();
  return out ? `?${out}` : "";
};
const enc = encodeURIComponent;

export const api = {
  // auth
  nonce: (wallet: string) => request<T.NonceResponse>("POST", "/v1/auth/nonce", { wallet } satisfies T.NonceRequest),
  verify: (body: T.VerifyRequest) => request<T.VerifyResponse>("POST", "/v1/auth/verify", body),

  // markets
  categories: () => request<T.Category[]>("GET", "/v1/categories"),
  markets: (p: { category?: string | null; q?: string | null; sort?: string | null; cursor?: string | null; limit?: number }) =>
    request<T.Page<T.MarketSummary>>("GET", `/v1/markets${qs(p)}`),
  market: (id: string) => request<T.MarketDetail>("GET", `/v1/markets/${enc(id)}`),
  history: (id: string, range: string) => request<T.MarketHistory>("GET", `/v1/markets/${enc(id)}/history${qs({ range })}`),
  trades: (id: string) => request<T.MarketTrade[]>("GET", `/v1/markets/${enc(id)}/trades`),

  // single-market orders
  orderQuote: (body: T.OrderQuoteRequest) => request<T.OrderQuote>("POST", "/v1/orders/quote", body),
  orderBuild: (quoteId: string) => request<T.BuildResponse>("POST", `/v1/orders/${enc(quoteId)}/build`),
  orderSubmit: (orderId: string, body: T.SubmitRequest) => request<T.OrderView>("POST", `/v1/orders/${enc(orderId)}/submit`, body),
  order: (orderId: string) => request<T.OrderView>("GET", `/v1/orders/${enc(orderId)}`),

  // baskets
  baskets: () => request<T.BasketTemplateSummary[]>("GET", "/v1/baskets"),
  basket: (slug: string) => request<T.BasketTemplateDetail>("GET", `/v1/baskets/${enc(slug)}`),
  basketQuote: (templateId: string, body: T.BasketQuoteRequest) => request<T.BasketQuote>("POST", `/v1/baskets/${enc(templateId)}/quote`, body),
  basketBuild: (quoteId: string) => request<T.BasketBuildResponse>("POST", `/v1/baskets/quotes/${enc(quoteId)}/build`),
  basketSubmit: (purchaseId: string, body: T.SubmitRequest) =>
    request<T.BasketPurchaseView>("POST", `/v1/baskets/purchases/${enc(purchaseId)}/submit`, body),
  purchase: (purchaseId: string) => request<T.BasketPurchaseView>("GET", `/v1/baskets/purchases/${enc(purchaseId)}`),

  // portfolio + claims
  portfolio: () => request<T.Portfolio>("GET", "/v1/portfolio"),
  activity: () => request<T.ActivityItem[]>("GET", "/v1/portfolio/activity"),
  claimsBuild: (body: T.ClaimsBuildRequest) => request<T.ClaimsBuildResponse>("POST", "/v1/claims/build", body),
  claimsSubmit: (body: T.ClaimsSubmitRequest) => request<T.ClaimsSubmitResponse>("POST", "/v1/claims/submit", body),

  // resolution center
  resolutionMarkets: (status: T.ResolutionStatus) => request<T.ResolutionMarketRow[]>("GET", `/v1/resolution/markets${qs({ status })}`),
  resolutionMarket: (id: string) => request<T.ResolutionMarketDetail>("GET", `/v1/resolution/markets/${enc(id)}`),
  creator: (creator: string) => request<T.CreatorDetail>("GET", `/v1/resolution/creators/${enc(creator)}`),

  // tokenized baskets (vault, ADR 0002)
  vaultBasket: (slug: string) => request<T.VaultBasketView>("GET", `/v1/vault/baskets/${enc(slug)}`),
  vaultDeposit: (templateId: string, body: T.VaultDepositRequest) => request<T.VaultBuildResponse>("POST", `/v1/vault/baskets/${enc(templateId)}/deposit`, body),
  vaultRedeem: (templateId: string, body: T.VaultRedeemRequest) => request<T.VaultBuildResponse>("POST", `/v1/vault/baskets/${enc(templateId)}/redeem`, body),
  vaultSubmit: (txId: string, body: T.SubmitRequest) => request<T.VaultTxView>("POST", `/v1/vault/txs/${enc(txId)}/submit`, body),
  vaultTx: (txId: string) => request<T.VaultTxView>("GET", `/v1/vault/txs/${enc(txId)}`),

  // admin
  adminMarkets: (q: string) => request<T.AdminPantaMarket[]>("GET", `/v1/admin/panta-markets${qs({ q })}`),
  adminCreate: (body: T.CreateBasketRequest) => request<T.AdminBasketView>("POST", "/v1/admin/baskets", body),
  adminUpdate: (id: string, body: T.UpdateBasketRequest) => request<T.AdminBasketView>("PATCH", `/v1/admin/baskets/${enc(id)}`, body),
  adminPublish: (id: string) => request<T.AdminBasketView>("POST", `/v1/admin/baskets/${enc(id)}/publish`),
  adminClose: (id: string) => request<T.AdminBasketView>("POST", `/v1/admin/baskets/${enc(id)}/close`),
  adminTokenize: (id: string) => request<T.VaultBuildResponse>("POST", `/v1/admin/baskets/${enc(id)}/tokenize`),
  adminVaultClose: (id: string) => request<T.VaultBuildResponse>("POST", `/v1/admin/baskets/${enc(id)}/vault-close`),
  adminVaultSettle: (id: string) => request<T.VaultBuildResponse>("POST", `/v1/admin/baskets/${enc(id)}/vault-settle`),
};

import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { api } from "./endpoints";
import type { ResolutionStatus } from "./types";
import { useSession } from "@/wallet/session";

export const qk = {
  categories: ["categories"] as const,
  markets: (p: object) => ["markets", p] as const,
  market: (id: string) => ["market", id] as const,
  history: (id: string, range: string) => ["history", id, range] as const,
  trades: (id: string) => ["trades", id] as const,
  baskets: ["baskets"] as const,
  basket: (slug: string) => ["basket", slug] as const,
  purchase: (id: string) => ["purchase", id] as const,
  portfolio: (w: string | null) => ["portfolio", w] as const,
  activity: (w: string | null) => ["activity", w] as const,
  resolution: (s: string) => ["resolution", s] as const,
  resolutionMarket: (id: string) => ["resolution-market", id] as const,
  creator: (c: string) => ["creator", c] as const,
};

export const useCategories = () => useQuery({ queryKey: qk.categories, queryFn: api.categories, staleTime: 10 * 60_000 });

export function useMarkets(p: { category?: string | null; q?: string | null; sort?: string | null }) {
  return useInfiniteQuery({
    queryKey: qk.markets(p),
    queryFn: ({ pageParam }) => api.markets({ ...p, cursor: pageParam, limit: 24 }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.next_cursor ?? null,
    staleTime: 30_000,
  });
}

export const useMarket = (id: string) => useQuery({ queryKey: qk.market(id), queryFn: () => api.market(id), refetchInterval: 30_000 });
export const useHistory = (id: string, range: string) =>
  useQuery({ queryKey: qk.history(id, range), queryFn: () => api.history(id, range), staleTime: 60_000 });
export const useTrades = (id: string) => useQuery({ queryKey: qk.trades(id), queryFn: () => api.trades(id), refetchInterval: 30_000 });

export const useBaskets = () => useQuery({ queryKey: qk.baskets, queryFn: api.baskets, staleTime: 30_000 });
export const useBasket = (slug: string) => useQuery({ queryKey: qk.basket(slug), queryFn: () => api.basket(slug), refetchInterval: 30_000 });

export function usePortfolio() {
  const { session } = useSession();
  return useQuery({ queryKey: qk.portfolio(session?.wallet ?? null), queryFn: api.portfolio, enabled: !!session, refetchInterval: 60_000 });
}
export function useActivity() {
  const { session } = useSession();
  return useQuery({ queryKey: qk.activity(session?.wallet ?? null), queryFn: api.activity, enabled: !!session });
}

export const useResolutionMarkets = (status: ResolutionStatus) =>
  useQuery({ queryKey: qk.resolution(status), queryFn: () => api.resolutionMarkets(status), staleTime: 60_000 });
export const useResolutionMarket = (id: string) => useQuery({ queryKey: qk.resolutionMarket(id), queryFn: () => api.resolutionMarket(id) });
export const useCreator = (c: string) => useQuery({ queryKey: qk.creator(c), queryFn: () => api.creator(c) });

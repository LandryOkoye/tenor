import { lazy, Suspense, type ReactNode } from "react";
import { createBrowserRouter, RouterProvider } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ApiError } from "@/api/client";
import { LandingPage } from "@/features/landing/LandingPage";
import { RouteError } from "@/components/layout/RouteError";

const AppLayout = lazy(() => import("@/components/layout/AppLayout"));
const MarketsPage = lazy(() => import("@/features/markets/MarketsPage"));
const MarketDetailPage = lazy(() => import("@/features/markets/MarketDetailPage"));
const BasketsPage = lazy(() => import("@/features/baskets/BasketsPage"));
const BasketDetailPage = lazy(() => import("@/features/baskets/BasketDetailPage"));
const PortfolioPage = lazy(() => import("@/features/portfolio/PortfolioPage"));
const ResolutionPage = lazy(() => import("@/features/resolution/ResolutionPage"));
const ResolutionMarketPage = lazy(() => import("@/features/resolution/ResolutionMarketPage"));
const CreatorPage = lazy(() => import("@/features/resolution/CreatorPage"));
const AdminPage = lazy(() => import("@/features/admin/AdminPage"));
const LearnPage = lazy(() => import("@/features/learn/LearnPage"));
const LegalPage = lazy(() => import("@/features/learn/LegalPage"));
const NotFound = lazy(() => import("@/components/layout/NotFound"));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (n, e) => !(e instanceof ApiError && e.status >= 400 && e.status < 500) && n < 2,
      refetchOnWindowFocus: false,
    },
  },
});

const s = (el: ReactNode) => <Suspense fallback={<PageFallback />}>{el}</Suspense>;

const router = createBrowserRouter([
  { path: "/", element: <LandingPage />, errorElement: <RouteError /> },
  {
    element: (
      <Suspense fallback={<PageFallback />}>
        <AppLayout />
      </Suspense>
    ),
    errorElement: <RouteError />,
    children: [
      { path: "/markets", element: s(<MarketsPage />) },
      { path: "/markets/:id", element: s(<MarketDetailPage />) },
      { path: "/baskets", element: s(<BasketsPage />) },
      { path: "/baskets/:slug", element: s(<BasketDetailPage />) },
      { path: "/portfolio", element: s(<PortfolioPage />) },
      { path: "/resolution", element: s(<ResolutionPage />) },
      { path: "/resolution/markets/:id", element: s(<ResolutionMarketPage />) },
      { path: "/resolution/creators/:creator", element: s(<CreatorPage />) },
      { path: "/admin", element: s(<AdminPage />) },
      { path: "/learn", element: s(<LearnPage />) },
      { path: "/legal", element: s(<LegalPage />) },
      { path: "*", element: s(<NotFound />) },
    ],
  },
]);

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}

function PageFallback() {
  return (
    <div className="grid min-h-[50dvh] place-items-center" role="status" aria-label="Loading">
      <span className="size-3 animate-pulse-dot rounded-full bg-accent" />
    </div>
  );
}

import { useEffect, useRef } from "react";
import { NavLink, Outlet, ScrollRestoration, useLocation } from "react-router";
import { Basket, ChartLineUp, Scales, Wallet as WalletIcon, BookOpen } from "@phosphor-icons/react";
import { WalletProviders } from "@/wallet/WalletProviders";
import { Logo } from "@/components/brand/Logo";
import { cn } from "@/lib/cn";
import { MOCK } from "@/lib/env";
import { WalletButton } from "./WalletButton";
import { SiteFooter } from "./SiteFooter";
import { AgeGate } from "./AgeGate";
import { Link } from "react-router";

export const NAV = [
  { to: "/markets", label: "Markets", icon: ChartLineUp },
  { to: "/baskets", label: "Baskets", icon: Basket },
  { to: "/portfolio", label: "Portfolio", icon: WalletIcon },
  { to: "/resolution", label: "Resolution", icon: Scales },
  { to: "/learn", label: "Learn", icon: BookOpen },
];

export default function AppLayout() {
  const { pathname } = useLocation();
  const main = useRef<HTMLElement>(null);
  // Move focus to the page on navigation for screen-reader users.
  useEffect(() => {
    main.current?.focus({ preventScroll: true });
  }, [pathname]);

  return (
    <WalletProviders>
      <a href="#main" className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[200] focus:rounded-full focus:bg-ink focus:px-4 focus:py-2 focus:text-paper">
        Skip to content
      </a>
      {MOCK && (
        <div className="bg-accent px-4 py-1.5 text-center text-xs font-medium text-ink">
          <span className="sm:hidden">Demo data · nothing is live</span>
          <span className="hidden sm:inline">Demo data: mock mode is on. Nothing here is live and no transactions are sent.</span>
        </div>
      )}
      <header className="sticky top-0 z-40 border-b border-line/70 bg-paper/85 backdrop-blur-md">
        <div className="mx-auto flex h-16 max-w-7xl items-center gap-6 px-4 sm:px-6">
          <Link to="/" aria-label="Tenor home" className="text-2xl">
            <Logo />
          </Link>
          <nav aria-label="Primary" className="hidden flex-1 md:block">
            <ul className="flex items-center gap-1 rounded-full border border-line bg-surface p-1 w-fit mx-auto shadow-soft">
              {NAV.map((n) => (
                <li key={n.to}>
                  <NavLink
                    to={n.to}
                    className={({ isActive }) =>
                      cn("block rounded-full px-4 py-2 text-sm font-medium transition-colors", isActive ? "bg-ink text-paper" : "text-muted hover:text-ink hover:bg-sunk")
                    }
                  >
                    {n.label}
                  </NavLink>
                </li>
              ))}
            </ul>
          </nav>
          <div className="ml-auto md:ml-0">
            <WalletButton />
          </div>
        </div>
      </header>

      <main id="main" ref={main} tabIndex={-1} className="mx-auto min-h-[70dvh] max-w-7xl px-4 pb-28 pt-6 outline-none sm:px-6 sm:pt-10 md:pb-20">
        <Outlet />
      </main>

      <SiteFooter />

      <nav aria-label="Primary mobile" className="fixed inset-x-0 bottom-0 z-40 border-t border-line bg-paper/95 pb-[env(safe-area-inset-bottom)] backdrop-blur-md md:hidden">
        <ul className="grid grid-cols-5">
          {NAV.map((n) => (
            <li key={n.to}>
              <NavLink
                to={n.to}
                className={({ isActive }) => cn("flex h-16 flex-col items-center justify-center gap-1 text-[11px] font-medium", isActive ? "text-ink" : "text-faint")}
              >
                {({ isActive }) => (
                  <>
                    <n.icon size={22} weight={isActive ? "fill" : "regular"} aria-hidden="true" />
                    {n.label}
                  </>
                )}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>
      <AgeGate />
      <ScrollRestoration />
    </WalletProviders>
  );
}

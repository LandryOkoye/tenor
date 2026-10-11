import { useSearchParams } from "react-router";
import { Scales } from "@phosphor-icons/react";
import type { ResolutionStatus } from "@/api/types";
import { useResolutionMarkets } from "@/api/queries";
import { Card, EmptyState, ErrorState, PoweredByPanta, Skeleton, Tabs } from "@/components/ui/primitives";
import { ResolutionRow } from "./parts";

const TABS: Array<{ value: ResolutionStatus; label: string; empty: string }> = [
  { value: "resolving_soon", label: "Resolving soon", empty: "No markets are due to resolve in the next few days." },
  { value: "resolved", label: "Resolved", empty: "No resolutions recorded yet." },
  { value: "delayed", label: "Delayed", empty: "Nothing is running late. Good." },
];

export default function ResolutionPage() {
  const [params, setParams] = useSearchParams();
  const status = (params.get("status") as ResolutionStatus) ?? "resolving_soon";
  const { data, isLoading, isError, error, refetch } = useResolutionMarkets(status);
  const tab = TABS.find((t) => t.value === status) ?? TABS[0];

  return (
    <div>
      <div className="grid gap-6 lg:grid-cols-[1.3fr_1fr] lg:items-end">
        <div>
          <p className="label-mono text-muted">Resolution Center</p>
          <h1 className="mt-2 text-4xl font-semibold leading-[1.02] tracking-[-0.045em] sm:text-5xl">How every market resolves, out in the open.</h1>
        </div>
        <p className="text-muted">
          Rules, sources and a step-by-step timeline for each market, plus every creator's track record. Panta decides outcomes; Tenor makes them easy to check.
        </p>
      </div>

      <div className="mt-8 flex flex-wrap items-center justify-between gap-3">
        <Tabs label="Resolution status" className="w-full sm:w-auto" value={status} onChange={(v) => setParams({ status: v }, { replace: true })} items={TABS.map((t) => ({ value: t.value, label: t.label }))} />
        <PoweredByPanta />
      </div>

      <div className="mt-5">
        {isError ? (
          <ErrorState error={error} onRetry={() => refetch()} />
        ) : isLoading ? (
          <Skeleton className="h-80" />
        ) : !data?.length ? (
          <EmptyState icon={<Scales size={24} />} title={tab.label} body={tab.empty} />
        ) : (
          <Card className="overflow-hidden">
            <ul className="divide-y divide-line">
              {data.map((r) => (
                <li key={r.market_id}><ResolutionRow r={r} /></li>
              ))}
            </ul>
          </Card>
        )}
      </div>
    </div>
  );
}

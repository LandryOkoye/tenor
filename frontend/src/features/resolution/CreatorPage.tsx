import { Link, useParams } from "react-router";
import { ArrowLeft } from "@phosphor-icons/react";
import { useCreator } from "@/api/queries";
import { Card, ErrorState, Skeleton } from "@/components/ui/primitives";
import { shortAddress } from "@/wallet/useTenorWallet";
import { ResolutionRow, Scorecard } from "./parts";

export default function CreatorPage() {
  const { creator = "" } = useParams();
  const { data, isLoading, isError, error, refetch } = useCreator(creator);
  if (isError) return <ErrorState error={error} onRetry={() => refetch()} />;
  if (isLoading || !data) return <Skeleton className="h-96" />;
  return (
    <div>
      <Link to="/resolution" className="inline-flex items-center gap-1.5 text-sm text-muted hover:text-ink">
        <ArrowLeft size={16} aria-hidden="true" /> Resolution Center
      </Link>
      <p className="label-mono mt-6 text-muted">Market creator</p>
      <h1 className="mt-2 font-mono text-2xl font-semibold tracking-tight sm:text-3xl">{shortAddress(creator, 8)}</h1>
      <div className="mt-8 grid gap-6 lg:grid-cols-[400px_1fr]">
        <Scorecard s={data.scorecard} link={false} />
        <Card className="overflow-hidden">
          <h2 className="px-5 pt-5 text-lg font-semibold tracking-tight">Their markets</h2>
          {data.markets.length === 0 ? (
            <p className="p-5 text-sm text-muted">No tracked markets yet.</p>
          ) : (
            <ul className="mt-3 divide-y divide-line border-t border-line">
              {data.markets.map((r) => (
                <li key={r.market_id}><ResolutionRow r={r} /></li>
              ))}
            </ul>
          )}
        </Card>
      </div>
    </div>
  );
}

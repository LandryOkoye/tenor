import { useTrades } from "@/api/queries";
import { ErrorState, SideBadge, Skeleton } from "@/components/ui/primitives";
import { formatPrice, formatUsdc, formatShares } from "@/lib/money";
import { relative } from "@/lib/time";

export function TradeTape({ marketId }: { marketId: string }) {
  const { data, isLoading, isError, error, refetch } = useTrades(marketId);
  if (isLoading) return <Skeleton className="h-48" />;
  if (isError) return <ErrorState error={error} onRetry={() => refetch()} />;
  if (!data?.length) return <p className="py-8 text-center text-sm text-muted">No trades yet. Be the first.</p>;
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <caption className="sr-only">Recent trades</caption>
        <thead>
          <tr className="text-left text-xs text-muted">
            <th scope="col" className="py-2 font-medium">Side</th>
            <th scope="col" className="py-2 text-right font-medium">Price</th>
            <th scope="col" className="py-2 text-right font-medium">Amount</th>
            <th scope="col" className="hidden py-2 text-right font-medium sm:table-cell">Shares</th>
            <th scope="col" className="py-2 text-right font-medium">When</th>
          </tr>
        </thead>
        <tbody className="num">
          {data.slice(0, 20).map((t, i) => (
            <tr key={t.signature ?? i} className="border-t border-line">
              <td className="py-2.5">{t.side ? <SideBadge side={t.side} /> : "–"}</td>
              <td className="py-2.5 text-right">{formatPrice(t.price).cents}</td>
              <td className="py-2.5 text-right font-medium">{formatUsdc(t.amount_usdc)}</td>
              <td className="hidden py-2.5 text-right text-muted sm:table-cell">{formatShares(t.shares)}</td>
              <td className="py-2.5 text-right text-muted">{relative(t.ts)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

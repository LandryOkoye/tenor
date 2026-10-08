import { isRouteErrorResponse, Link, useRouteError } from "react-router";
import { Mark } from "@/components/brand/Logo";

export function RouteError() {
  const err = useRouteError();
  const msg = isRouteErrorResponse(err) ? `${err.status} ${err.statusText}` : err instanceof Error ? err.message : "Unknown error";
  return (
    <main className="grid min-h-dvh place-items-center bg-paper px-6 text-center">
      <div>
        <Mark className="mx-auto h-10" />
        <h1 className="mt-6 text-3xl font-semibold tracking-tight">Something broke on our side.</h1>
        <p className="mt-2 text-muted">{msg}</p>
        <Link to="/" className="mt-6 inline-flex h-11 items-center rounded-full bg-ink px-5 text-paper">Back home</Link>
      </div>
    </main>
  );
}

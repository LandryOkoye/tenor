import { ButtonLink } from "@/components/ui/Button";

export default function NotFound() {
  return (
    <div className="mx-auto max-w-xl py-24 text-center">
      <p className="label-mono text-muted">404</p>
      <h1 className="mt-3 text-4xl font-semibold tracking-tight">This market doesn't exist.</h1>
      <p className="mt-2 text-muted">It may have been removed, or the link is wrong.</p>
      <ButtonLink to="/markets" className="mt-8">Browse markets</ButtonLink>
    </div>
  );
}

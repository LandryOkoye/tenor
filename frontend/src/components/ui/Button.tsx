import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { Link, type LinkProps } from "react-router";
import { cn } from "@/lib/cn";

type Variant = "primary" | "accent" | "secondary" | "ghost" | "yes" | "no" | "inverse";
type Size = "sm" | "md" | "lg";

const base =
  "inline-flex items-center justify-center gap-2 rounded-full font-medium transition-[background-color,color,box-shadow,transform] duration-150 active:scale-[0.98] disabled:opacity-45 disabled:cursor-not-allowed disabled:active:scale-100 select-none whitespace-nowrap";
const variants: Record<Variant, string> = {
  primary: "bg-ink text-paper hover:bg-ink-3 shadow-[inset_0_1px_0_rgb(255_255_255/0.12)]",
  accent: "bg-accent text-ink hover:brightness-95",
  secondary: "bg-surface text-ink border border-line hover:border-line-strong hover:bg-sunk",
  ghost: "text-ink hover:bg-sunk",
  yes: "bg-yes text-white hover:brightness-110",
  no: "bg-no text-white hover:brightness-110",
  inverse: "bg-paper text-ink hover:bg-white",
};
const sizes: Record<Size, string> = {
  sm: "h-9 px-4 text-sm",
  md: "h-11 px-5 text-[15px]",
  lg: "h-13 px-7 text-base",
};

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: Size; loading?: boolean };

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "primary", size = "md", loading, className, children, disabled, ...rest },
  ref,
) {
  return (
    <button ref={ref} className={cn(base, variants[variant], sizes[size], className)} disabled={disabled || loading} aria-busy={loading || undefined} {...rest}>
      {loading && <Spinner />}
      {children}
    </button>
  );
});

export function ButtonLink({ variant = "primary", size = "md", className, children, ...rest }: LinkProps & { variant?: Variant; size?: Size; children: ReactNode }) {
  return (
    <Link className={cn(base, variants[variant], sizes[size], className)} {...rest}>
      {children}
    </Link>
  );
}

export function Spinner({ className }: { className?: string }) {
  return <span className={cn("inline-block size-4 animate-spin rounded-full border-2 border-current border-r-transparent", className)} aria-hidden="true" />;
}

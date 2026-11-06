import type { ButtonHTMLAttributes } from "react";
import { cx } from "./cx";
import { Spinner } from "./Spinner";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

const VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-accent text-on-accent hover:brightness-105 disabled:bg-raised disabled:text-muted disabled:hover:brightness-100",
  secondary: "border-control bg-surface text-ink hover:bg-raised disabled:text-muted",
  ghost: "text-ink hover:bg-raised disabled:text-muted",
  danger: "border-danger bg-surface text-danger hover:bg-raised disabled:border-control disabled:text-muted",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "h-8 rounded-control-sm px-3 text-body",
  md: "h-10 rounded-control px-[18px] text-body",
  lg: "h-12 rounded-control-lg px-7 text-section",
};

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Mostra o indicador de carregamento e bloqueia novos cliques, mantendo o texto. */
  loading?: boolean;
};

export function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  disabled,
  type = "button",
  className,
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={cx(
        "inline-flex items-center justify-center gap-2 whitespace-nowrap border border-transparent font-semibold transition-colors",
        "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:cursor-not-allowed",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...rest}
    >
      {loading && <Spinner size={16} label={null} />}
      {children}
    </button>
  );
}

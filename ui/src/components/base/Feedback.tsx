import type { ReactNode } from "react";
import { cx } from "./cx";

type Props = {
  title: string;
  /** O que fazer agora, em linguagem simples. */
  description?: ReactNode;
  icon?: ReactNode;
  action?: ReactNode;
  className?: string;
};

/** Tela ou área sem conteúdo: diz por que está vazia e qual o próximo passo. */
export function EmptyState({ title, description, icon, action, className }: Props) {
  return (
    <div className={cx("flex flex-col items-center gap-3 px-6 py-10 text-center", className)}>
      {icon && (
        <span aria-hidden="true" className="flex h-16 w-16 items-center justify-center rounded-[18px] bg-raised text-ink">
          {icon}
        </span>
      )}
      <h2 className="text-[1.375rem] font-semibold tracking-[-0.01em] text-ink">{title}</h2>
      {description && <p className="max-w-[22rem] text-body text-muted">{description}</p>}
      {action}
    </div>
  );
}

/** Falha anunciada a leitores de tela: o que aconteceu e como resolver, sem pedir desculpas. */
export function ErrorMessage({ title, description, action, className }: Omit<Props, "icon">) {
  return (
    <div role="alert" className={cx("flex flex-col gap-2 rounded-card border border-danger bg-surface p-4", className)}>
      <p className="text-body font-semibold text-danger">{title}</p>
      {description && <p className="text-body text-ink">{description}</p>}
      {action && <div>{action}</div>}
    </div>
  );
}

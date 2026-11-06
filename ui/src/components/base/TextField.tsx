import { useId, type InputHTMLAttributes, type ReactNode } from "react";
import { cx } from "./cx";

type Props = Omit<InputHTMLAttributes<HTMLInputElement>, "size"> & {
  label: string;
  /** Esconde o rótulo visualmente; ele continua disponível para leitores de tela. */
  hideLabel?: boolean;
  /** Mensagem de erro: diz o que aconteceu e como resolver. */
  error?: string;
  icon?: ReactNode;
};

export function TextField({ label, hideLabel = false, error, icon, id, className, ...rest }: Props) {
  const autoId = useId();
  const inputId = id ?? autoId;
  const errorId = `${inputId}-erro`;
  return (
    <div className={cx("flex flex-col gap-1", className)}>
      <label htmlFor={inputId} className={hideLabel ? "sr-only" : "text-caption font-semibold text-ink"}>
        {label}
      </label>
      <div
        className={cx(
          "flex h-10 items-center gap-2 rounded-control border bg-surface px-3 text-ink",
          "focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-accent",
          error ? "border-danger" : "border-control",
        )}
      >
        {icon && <span aria-hidden="true" className="flex text-muted">{icon}</span>}
        <input
          id={inputId}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? errorId : undefined}
          className="min-w-0 flex-1 bg-transparent text-body outline-none placeholder:text-muted"
          {...rest}
        />
      </div>
      {error && <p id={errorId} className="text-caption text-danger">{error}</p>}
    </div>
  );
}

const searchIcon = (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
    <circle cx="11" cy="11" r="7" />
    <path d="M20 20l-4-4" />
  </svg>
);

/** Campo de busca: rótulo escondido por padrão, ícone de lupa. */
export function SearchField(props: Omit<Props, "icon" | "type">) {
  return <TextField hideLabel type="search" icon={searchIcon} {...props} />;
}

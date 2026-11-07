import type { ReactNode } from "react";
import { cx } from "./cx";

export type SegmentedOption<T extends string> = { value: T; label: ReactNode; ariaLabel?: string };

type Props<T extends string> = {
  label: string;
  options: SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string;
};

/** Escolha única entre poucas opções (tema, posição, aba). */
export function Segmented<T extends string>({ label, options, value, onChange, className }: Props<T>) {
  return (
    <div role="group" aria-label={label} className={cx("inline-flex gap-0.5 rounded-control bg-raised p-[3px]", className)}>
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            aria-label={option.ariaLabel}
            title={option.ariaLabel}
            onClick={() => onChange(option.value)}
            className={cx(
              "inline-flex h-8 items-center justify-center gap-1.5 rounded-control-sm px-3 text-body font-medium",
              "focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent",
              selected ? "bg-surface text-ink shadow-[0_0_0_1px_var(--el-line)]" : "text-muted hover:text-ink",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

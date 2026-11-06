import type { ThemePreference } from "../theme/theme";

const OPTIONS: { value: ThemePreference; label: string }[] = [
  { value: "sistema", label: "Sistema" },
  { value: "claro", label: "Claro" },
  { value: "escuro", label: "Escuro" },
];

type Props = { preference: ThemePreference; onChange: (preference: ThemePreference) => void };

/** Seletor provisório; o lugar final é a barra de título própria (#126). */
export function ThemeSwitcher({ preference, onChange }: Props) {
  return (
    <div role="group" aria-label="Tema" className="flex gap-1">
      {OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={preference === option.value}
          onClick={() => onChange(option.value)}
          className="rounded border border-slate-600 px-2 py-1 text-sm aria-pressed:border-sky-400 aria-pressed:bg-sky-900"
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

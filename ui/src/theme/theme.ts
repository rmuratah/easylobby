import { useCallback, useEffect, useState } from "react";

export type Theme = "claro" | "escuro";
export type ThemePreference = "sistema" | Theme;

/** Mesma chave usada por public/theme-init.js, que aplica o tema antes do React. */
export const THEME_STORAGE_KEY = "easylobby.tema";
const DARK_QUERY = "(prefers-color-scheme: dark)";

export function isThemePreference(value: unknown): value is ThemePreference {
  return value === "sistema" || value === "claro" || value === "escuro";
}

export function readPreference(): ThemePreference {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return isThemePreference(stored) ? stored : "sistema";
  } catch {
    return "sistema";
  }
}

function savePreference(preference: ThemePreference) {
  try {
    if (preference === "sistema") localStorage.removeItem(THEME_STORAGE_KEY);
    else localStorage.setItem(THEME_STORAGE_KEY, preference);
  } catch {
    // Sem armazenamento, a escolha vale só até fechar o app.
  }
}

function systemIsDark(): boolean {
  return typeof matchMedia === "function" && matchMedia(DARK_QUERY).matches;
}

export function resolveTheme(preference: ThemePreference, systemDark: boolean): Theme {
  if (preference === "sistema") return systemDark ? "escuro" : "claro";
  return preference;
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
}

/** Preferência de tema do usuário; em "sistema", acompanha o Windows com o app aberto. */
export function useTheme() {
  const [preference, setPreferenceState] = useState<ThemePreference>(readPreference);
  const [systemDark, setSystemDark] = useState(systemIsDark);
  const theme = resolveTheme(preference, systemDark);

  useEffect(() => {
    if (typeof matchMedia !== "function") return;
    const query = matchMedia(DARK_QUERY);
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const setPreference = useCallback((next: ThemePreference) => {
    savePreference(next);
    setPreferenceState(next);
  }, []);

  return { preference, theme, setPreference };
}

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeSwitcher } from "../components/ThemeSwitcher";
import { readPreference, resolveTheme, THEME_STORAGE_KEY, useTheme } from "./theme";

type Listener = (event: MediaQueryListEvent) => void;

function mockSystem(initialDark: boolean) {
  let dark = initialDark;
  const listeners = new Set<Listener>();
  vi.stubGlobal("matchMedia", (query: string) => ({
    get matches() { return query === "(prefers-color-scheme: dark)" && dark; },
    media: query,
    addEventListener: (_type: string, listener: Listener) => listeners.add(listener),
    removeEventListener: (_type: string, listener: Listener) => listeners.delete(listener),
  }));
  return {
    set(nextDark: boolean) {
      dark = nextDark;
      listeners.forEach((listener) => listener({ matches: nextDark } as MediaQueryListEvent));
    },
  };
}

beforeEach(() => {
  localStorage.clear();
  delete document.documentElement.dataset.theme;
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("resolveTheme", () => {
  it("segue o sistema em 'sistema' e respeita a escolha manual", () => {
    expect(resolveTheme("sistema", true)).toBe("escuro");
    expect(resolveTheme("sistema", false)).toBe("claro");
    expect(resolveTheme("claro", true)).toBe("claro");
    expect(resolveTheme("escuro", false)).toBe("escuro");
  });
});

describe("readPreference", () => {
  it("volta para 'sistema' com valor desconhecido ou sem armazenamento", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "roxo");
    expect(readPreference()).toBe("sistema");
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("bloqueado"); });
    expect(readPreference()).toBe("sistema");
  });
});

describe("useTheme", () => {
  it("acompanha a troca de tema do Windows com o app aberto", () => {
    const system = mockSystem(true);
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe("escuro");
    expect(document.documentElement.dataset.theme).toBe("escuro");

    act(() => system.set(false));
    expect(result.current.theme).toBe("claro");
    expect(document.documentElement.dataset.theme).toBe("claro");
  });

  it("salva a escolha manual e ignora o Windows enquanto ela vale", () => {
    const system = mockSystem(false);
    const { result } = renderHook(() => useTheme());
    act(() => result.current.setPreference("escuro"));
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("escuro");
    expect(document.documentElement.dataset.theme).toBe("escuro");

    act(() => system.set(false));
    expect(document.documentElement.dataset.theme).toBe("escuro");

    act(() => result.current.setPreference("sistema"));
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBeNull();
    expect(document.documentElement.dataset.theme).toBe("claro");
  });

  it("retoma a escolha salva ao abrir de novo", () => {
    mockSystem(true);
    localStorage.setItem(THEME_STORAGE_KEY, "claro");
    const { result } = renderHook(() => useTheme());
    expect(result.current.preference).toBe("claro");
    expect(document.documentElement.dataset.theme).toBe("claro");
  });
});

describe("theme-init.js", () => {
  // Vitest roda a partir de ui/.
  const script = readFileSync(resolve(process.cwd(), "public/theme-init.js"), "utf8");

  it("usa a mesma chave de armazenamento do app", () => {
    expect(script).toContain(`"${THEME_STORAGE_KEY}"`);
  });

  it("aplica o tema salvo ou o do sistema antes do React", () => {
    mockSystem(true);
    new Function(script)();
    expect(document.documentElement.dataset.theme).toBe("escuro");

    localStorage.setItem(THEME_STORAGE_KEY, "claro");
    new Function(script)();
    expect(document.documentElement.dataset.theme).toBe("claro");
  });
});

describe("ThemeSwitcher", () => {
  it("marca a opção atual e avisa a escolhida", () => {
    const onChange = vi.fn();
    render(<ThemeSwitcher preference="sistema" onChange={onChange} />);
    expect(screen.getByRole("button", { name: "Sistema" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "Escuro" }));
    expect(onChange).toHaveBeenCalledWith("escuro");
  });
});

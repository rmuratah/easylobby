import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// O Vitest roda a partir de ui/ (pnpm test ou pnpm --filter easylobby-ui test).
const css = readFileSync(resolve(process.cwd(), "src/theme/tokens.css"), "utf8");

function block(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`bloco ${selector} não encontrado`);
  const body = css.slice(start, css.indexOf("}", start));
  return Object.fromEntries(
    [...body.matchAll(/--el-([\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]),
  );
}

const claro = block('[data-theme="claro"]');
const escuro = block('[data-theme="escuro"]');

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// Pares [frente, fundo, mínimo WCAG AA]: 4.5 para texto, 3 para bordas e ícones.
const pairs: [string, string, number][] = [
  ["ink", "bg", 4.5],
  ["ink", "surface", 4.5],
  ["ink", "raised", 4.5],
  ["muted", "bg", 4.5],
  ["muted", "surface", 4.5],
  ["muted", "raised", 4.5],
  ["on-accent", "accent", 4.5],
  ["accent-ink", "accent-soft", 4.5],
  // Usos reais conferidos na validação do redesign (#133).
  ["ink", "accent-soft", 4.5],
  ["muted", "accent-soft", 4.5],
  ["warn", "accent-soft", 4.5],
  ["accent-ink", "surface", 4.5],
  ["accent-ink", "bg", 4.5],
  ["accent-ink", "raised", 4.5],
  ["danger", "bg", 4.5],
  ["danger", "raised", 4.5],
  ["danger", "surface", 4.5],
  ["warn", "surface", 4.5],
  ["control", "surface", 3],
  ["control", "bg", 3],
  ["accent", "surface", 3],
  ["status-online", "surface", 3],
  ["status-game", "surface", 3],
  ["status-away", "surface", 3],
];

describe.each([
  ["claro", claro],
  ["escuro", escuro],
])("tema %s", (_name, theme) => {
  it("define os mesmos tokens que o outro tema", () => {
    expect(Object.keys(theme).sort()).toEqual(Object.keys(claro).sort());
  });

  it.each(pairs)("%s sobre %s passa o contraste mínimo de %s:1", (fg, bg, min) => {
    expect(contrast(theme[fg], theme[bg])).toBeGreaterThanOrEqual(min);
  });
});

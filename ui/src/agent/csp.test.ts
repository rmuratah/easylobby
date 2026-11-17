import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { assetUrl } from './assets';
import { AGENT_URL } from './client';

// O Tauri só aplica a CSP no build de release; no dev (Vite) ela não vale.
// Este teste pega o que só apareceria no app instalado.
const conf = JSON.parse(readFileSync(resolve(process.cwd(), '../desktop/src-tauri/tauri.conf.json'), 'utf8'));
const csp: string = conf.app.security.csp;

function sources(directive: string): string[] {
  const parts = csp.split(';').map((p) => p.trim().split(/\s+/));
  const found = parts.find(([name]) => name === directive) ?? parts.find(([name]) => name === 'default-src');
  return found ? found.slice(1) : [];
}

function allows(directive: string, url: string): boolean {
  const { protocol, hostname } = new URL(url);
  return sources(directive).some((src) => src === `${protocol}//${hostname}:*` || src === `${protocol}//${hostname}`);
}

describe('CSP do app instalado', () => {
  it('libera as imagens servidas pelo agente', () => {
    expect(allows('img-src', assetUrl('champion', 1))).toBe(true);
    expect(allows('img-src', assetUrl('profileIcon', 29))).toBe(true);
  });

  it('libera o /ws do agente', () => {
    expect(allows('connect-src', AGENT_URL)).toBe(true);
  });
});

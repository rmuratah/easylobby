import { invoke, isTauri } from '@tauri-apps/api/core';
import type { GameflowPhase } from '../bindings/GameflowPhase';

/**
 * Atualização automática do app (#35). Quem consulta o GitHub Releases,
 * confere a assinatura e roda o instalador é o shell Tauri, não a UI.
 */
export type UpdateInfo = {
  /** Versão instalada. */
  currentVersion: string;
  /** Versão disponível. */
  version: string;
  /** Notas da release, quando houver. */
  notes: string | null;
};

export type AppUpdate = {
  /** Versão instalada. */
  version(): Promise<string>;
  /** Versão nova, ou `null` quando o app já está na mais recente. */
  check(): Promise<UpdateInfo | null>;
  /** Baixa e instala a versão do último `check`; o app fecha e abre de novo. */
  install(): Promise<void>;
};

/** Fora do Tauri (Vite no navegador) não há o que atualizar. */
export function tauriAppUpdate(): AppUpdate | null {
  if (!isTauri()) return null;
  return {
    version: () => invoke<string>('app_version'),
    check: () => invoke<UpdateInfo | null>('check_update'),
    install: () => invoke<void>('install_update'),
  };
}

/**
 * Fases em que dá para fechar o app para instalar. Na fila, na partida
 * encontrada, na seleção e na partida o jogador perderia a vez ou levaria
 * penalidade enquanto o app reinicia. `null` é o LoL fechado.
 */
const safePhases: ReadonlySet<GameflowPhase | null> = new Set<GameflowPhase | null>([
  null, 'None', 'Lobby', 'EndOfGame',
]);

export function canInstallNow(phase: GameflowPhase | null): boolean {
  return safePhases.has(phase);
}

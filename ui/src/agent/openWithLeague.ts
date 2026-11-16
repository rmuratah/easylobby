import { invoke, isTauri } from '@tauri-apps/api/core';

/**
 * Opção "Abrir junto com o LoL" (#511): o easylobby inicia com o Windows,
 * escondido na bandeja, e mostra a janela quando o LoL abre. Quem guarda a
 * escolha é o shell Tauri (a chave Run do registro), não a UI.
 */
export type OpenWithLeague = {
  isEnabled(): Promise<boolean>;
  /** Liga ou desliga e devolve o valor que ficou. */
  setEnabled(enabled: boolean): Promise<boolean>;
};

/** Fora do Tauri (Vite no navegador) não há o que ligar. */
export function tauriOpenWithLeague(): OpenWithLeague | null {
  if (!isTauri()) return null;
  return {
    isEnabled: () => invoke<boolean>('open_with_league_enabled'),
    setEnabled: (enabled) => invoke<boolean>('set_open_with_league', { enabled }),
  };
}

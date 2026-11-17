import { invoke, isTauri } from '@tauri-apps/api/core';

/**
 * Opção "Enviar erros para os devs" (#550): no app instalado, os erros do
 * log também vão para a base dos devs. Quem guarda a escolha e envia é o
 * shell Tauri, não a UI.
 */
export type ErrorReports = {
  /** `null` quando esta build não envia erros (desenvolvimento). */
  isEnabled(): Promise<boolean | null>;
  /** Liga ou desliga e devolve o valor que ficou. */
  setEnabled(enabled: boolean): Promise<boolean>;
};

/** Fora do Tauri (Vite no navegador) não há envio. */
export function tauriErrorReports(): ErrorReports | null {
  if (!isTauri()) return null;
  return {
    isEnabled: () => invoke<boolean | null>('error_reports_enabled'),
    setEnabled: (enabled) => invoke<boolean>('set_error_reports_enabled', { enabled }),
  };
}

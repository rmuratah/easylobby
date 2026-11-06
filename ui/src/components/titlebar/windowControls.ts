import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Ações da janela usadas pela barra de título. Fora do Tauri (Vite no navegador) não há janela para controlar. */
export type WindowControls = {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  /** Pede para fechar; o shell esconde a janela na bandeja em vez de sair. */
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  /** Avisa quando o tamanho muda (maximizar, restaurar, Snap). Devolve a função que para de ouvir. */
  onResized(listener: () => void): Promise<() => void>;
};

export function tauriWindowControls(): WindowControls | null {
  if (!isTauri()) return null;
  const appWindow = getCurrentWindow();
  return {
    minimize: () => appWindow.minimize(),
    toggleMaximize: () => appWindow.toggleMaximize(),
    close: () => appWindow.close(),
    isMaximized: () => appWindow.isMaximized(),
    onResized: (listener) => appWindow.onResized(() => listener()),
  };
}

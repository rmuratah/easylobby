import { invoke, isTauri } from '@tauri-apps/api/core';

/**
 * Log de erros do app (#534). Quem grava o arquivo é o shell Tauri, na pasta
 * de logs do app (um arquivo por dia). A UI só manda os erros dela e pede
 * para abrir a pasta. Nunca mande aqui o texto de uma mensagem do chat.
 */
export type ErrorLog = {
  report(kind: string, message: string, stack?: string): void;
  openFolder(): Promise<void>;
};

/** Fora do Tauri (Vite no navegador) não há arquivo de log. */
export function tauriErrorLog(): ErrorLog | null {
  if (!isTauri()) return null;
  return {
    report: (kind, message, stack) => {
      invoke('log_ui_error', { kind, message, stack: stack ?? null }).catch(() => {});
    },
    openFolder: () => invoke<void>('open_logs_folder'),
  };
}

let current: ErrorLog | null | undefined;

function log(): ErrorLog | null {
  if (current === undefined) current = tauriErrorLog();
  return current;
}

/** Troca o destino dos erros nos testes. */
export function setErrorLog(next: ErrorLog | null | undefined) {
  current = next;
}

function describe(error: unknown): { message: string; stack?: string } {
  if (error instanceof Error) return { message: `${error.name}: ${error.message}`, stack: error.stack };
  if (typeof error === 'string') return { message: error };
  try {
    return { message: JSON.stringify(error) ?? String(error) };
  } catch {
    return { message: String(error) };
  }
}

/** Grava um erro no log do app. Nunca lança. */
export function reportError(kind: string, error: unknown) {
  try {
    const { message, stack } = describe(error);
    log()?.report(kind, message, stack);
  } catch {
    // O log nunca pode derrubar a tela.
  }
}

/** Grava as exceções e as promessas rejeitadas sem tratamento da janela. */
export function installErrorLogging(target: Window = window): () => void {
  const onError = (event: ErrorEvent) => {
    reportError('exceção', event.error ?? `${event.message} (${event.filename}:${event.lineno})`);
  };
  const onRejection = (event: PromiseRejectionEvent) => reportError('promessa rejeitada', event.reason);
  target.addEventListener('error', onError);
  target.addEventListener('unhandledrejection', onRejection);
  return () => {
    target.removeEventListener('error', onError);
    target.removeEventListener('unhandledrejection', onRejection);
  };
}

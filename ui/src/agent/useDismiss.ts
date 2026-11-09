import { useEffect, useRef } from 'react';

/** Quanto tempo uma mensagem de erro fica na tela antes de sumir sozinha (#335). */
export const ERROR_DISMISS_MS = 6000;

/**
 * Chama `clear` depois de {@link ERROR_DISMISS_MS} enquanto `active` estiver
 * ligado (#335). Uma mensagem nova (outra `key`) recomeça a contagem; se o
 * erro sair antes (outra ação), o temporizador é cancelado.
 */
export function useDismiss(active: boolean, key: string, clear: () => void, delay = ERROR_DISMISS_MS) {
  const clearRef = useRef(clear);
  useEffect(() => { clearRef.current = clear; });
  useEffect(() => {
    if (!active) return;
    const timer = setTimeout(() => clearRef.current(), delay);
    return () => clearTimeout(timer);
  }, [active, key, delay]);
}

import { useCallback, useEffect, useState } from 'react';
import { tauriAppUpdate, type AppUpdate, type UpdateInfo } from './appUpdate';

/** De quanto em quanto tempo o app procura uma versão nova sozinho. */
export const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

export type AppUpdateState = {
  /** `null` fora do app desktop: a seção Atualizações some. */
  api: AppUpdate | null;
  /** Versão instalada, quando já chegou. */
  version: string | null;
  /** Versão nova encontrada. */
  available: UpdateInfo | null;
  status: 'idle' | 'checking' | 'upToDate' | 'installing' | 'checkFailed' | 'installFailed';
  check: () => void;
  install: () => void;
};

/**
 * Procura atualização ao abrir o app e a cada [`CHECK_INTERVAL_MS`] (#35).
 * Fica no `App` para o ponto de aviso no botão de Configurações aparecer
 * mesmo com o painel fechado.
 */
export function useAppUpdate(apiProp?: AppUpdate | null): AppUpdateState {
  const [api] = useState(() => apiProp === undefined ? tauriAppUpdate() : apiProp);
  const [version, setVersion] = useState<string | null>(null);
  const [available, setAvailable] = useState<UpdateInfo | null>(null);
  // A primeira consulta já começa ao abrir o app.
  const [status, setStatus] = useState<AppUpdateState['status']>(() => api ? 'checking' : 'idle');

  /** Consulta sem mexer no estado antes da resposta (a periódica não pisca o botão). */
  const query = useCallback(() => {
    if (!api) return;
    // Uma consulta periódica que volta durante a instalação não tira o "Instalando…".
    const settle = (next: AppUpdateState['status']) => setStatus(current => current === 'installing' ? current : next);
    api.check()
      .then(info => {
        setAvailable(info);
        settle(info ? 'idle' : 'upToDate');
      })
      .catch(() => settle('checkFailed'));
  }, [api]);

  const check = useCallback(() => {
    if (!api) return;
    setStatus('checking');
    query();
  }, [api, query]);

  useEffect(() => {
    if (!api) return;
    api.version().then(setVersion).catch(() => {});
    query();
    const timer = setInterval(query, CHECK_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [api, query]);

  const install = useCallback(() => {
    if (!api || !available) return;
    setStatus('installing');
    // Se der certo, o app fecha antes de a promessa voltar.
    api.install().catch(() => setStatus('installFailed'));
  }, [api, available]);

  return { api, version, available, status, check, install };
}

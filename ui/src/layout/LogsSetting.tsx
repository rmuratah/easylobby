import { useState } from 'react';
import { tauriErrorLog, type ErrorLog } from '../agent/errorLog';

type Props = {
  /** Troca o acesso ao shell nos testes. */
  api?: Pick<ErrorLog, 'openFolder'> | null;
};

/**
 * Botão "Abrir pasta de logs" do painel de Configurações (#534). O app grava
 * os erros num arquivo por dia; o botão abre a pasta no Explorer. Some fora
 * do app desktop.
 */
export function LogsSetting({ api: apiProp }: Props) {
  const [api] = useState(() => apiProp === undefined ? tauriErrorLog() : apiProp);
  const [failed, setFailed] = useState(false);
  if (!api) return null;

  async function open() {
    if (!api) return;
    setFailed(false);
    try {
      await api.openFolder();
    } catch {
      setFailed(true);
    }
  }

  return (
    <div className="flex flex-col gap-1 border-t border-line pt-3">
      <h2 className="mb-1 text-section font-semibold">Logs</h2>
      <button type="button" onClick={() => void open()}
        className="self-start rounded-full border border-control px-3 py-1 text-body transition-colors hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
        Abrir pasta de logs
      </button>
      <p className="text-caption text-muted">Os erros do app ficam salvos aqui, um arquivo por dia, por 7 dias.</p>
      {failed && <p role="alert" className="text-caption text-danger">Não deu para abrir a pasta agora.</p>}
    </div>
  );
}

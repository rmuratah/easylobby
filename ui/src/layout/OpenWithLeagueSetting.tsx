import { useEffect, useState } from 'react';
import { tauriOpenWithLeague, type OpenWithLeague } from '../agent/openWithLeague';

type Props = {
  /** Troca o acesso ao shell nos testes. */
  api?: OpenWithLeague | null;
};

/**
 * Liga e desliga "Abrir junto com o LoL" (#511) no painel de Configurações.
 * Some fora do app desktop e enquanto o valor atual não chegou.
 */
export function OpenWithLeagueSetting({ api: apiProp }: Props) {
  const [api] = useState(() => apiProp === undefined ? tauriOpenWithLeague() : apiProp);
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!api) return;
    let active = true;
    api.isEnabled()
      .then(value => { if (active) setEnabled(value); })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [api]);

  if (!api || (enabled === null && !failed)) return null;

  async function change(next: boolean) {
    if (!api) return;
    setSaving(true);
    setFailed(false);
    try {
      setEnabled(await api.setEnabled(next));
    } catch {
      setFailed(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex flex-col gap-1 border-t border-line pt-3">
      <h2 className="mb-1 text-section font-semibold">Iniciar</h2>
      <label className="flex items-center justify-between gap-3 text-body">
        Abrir junto com o LoL
        <input type="checkbox" role="switch" checked={enabled ?? false} disabled={saving}
          className="size-4 accent-[var(--el-accent)]"
          onChange={event => void change(event.target.checked)} />
      </label>
      <p className="text-caption text-muted">Inicia com o Windows na bandeja e abre quando o LoL abrir.</p>
      {failed && <p role="alert" className="text-caption text-danger">Não deu para mudar essa opção agora.</p>}
    </div>
  );
}

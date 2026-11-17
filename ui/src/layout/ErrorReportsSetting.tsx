import { useEffect, useState } from 'react';
import { tauriErrorReports, type ErrorReports } from '../agent/errorReports';

type Props = {
  /** Troca o acesso ao shell nos testes. */
  api?: ErrorReports | null;
};

/**
 * Liga e desliga "Enviar erros para os devs" (#550) no painel de
 * Configurações, com o aviso do que é enviado. Some fora do app desktop, na
 * build de desenvolvimento e enquanto o valor atual não chegou.
 */
export function ErrorReportsSetting({ api: apiProp }: Props) {
  const [api] = useState(() => apiProp === undefined ? tauriErrorReports() : apiProp);
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!api) return;
    let active = true;
    api.isEnabled()
      .then(value => { if (active) setEnabled(value); })
      .catch(() => {});
    return () => { active = false; };
  }, [api]);

  if (!api || enabled === null) return null;

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
    <div className="flex flex-col gap-1">
      <label className="flex items-center justify-between gap-3 text-body">
        Enviar erros para os devs
        <input type="checkbox" role="switch" checked={enabled} disabled={saving}
          className="size-4 accent-[var(--el-accent)]"
          onChange={event => void change(event.target.checked)} />
      </label>
      <p className="text-caption text-muted">
        Durante o alfa, os erros do app vão para os devs com a versão do app e um código aleatório da instalação.
        Nunca vão seu nick, senhas, chaves nem o chat.
      </p>
      {failed && <p role="alert" className="text-caption text-danger">Não deu para mudar essa opção agora.</p>}
    </div>
  );
}

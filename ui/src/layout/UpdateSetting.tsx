import type { AppUpdateState } from '../agent/useAppUpdate';

type Props = {
  update: AppUpdateState;
  /** Fora da fila, da seleção e da partida, quando dá para o app reiniciar. */
  canInstall: boolean;
};

const buttonClass = 'self-start rounded-full border border-control px-3 py-1 text-body transition-colors hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-60';

/**
 * Seção Atualizações do painel de Configurações (#35): versão instalada,
 * aviso de versão nova e o botão que instala e reinicia o app. Some fora do
 * app desktop.
 */
export function UpdateSetting({ update, canInstall }: Props) {
  const { api, version, available, status } = update;
  if (!api) return null;
  const busy = status === 'checking' || status === 'installing';

  return (
    <section aria-labelledby="update-heading" className="flex flex-col gap-1 border-t border-line pt-3">
      <h2 id="update-heading" className="mb-1 text-section font-semibold">Atualizações</h2>
      {version && <p className="text-caption text-muted">Versão instalada: {version}</p>}
      {available ? (
        <>
          <p className="text-body">Versão {available.version} disponível.</p>
          {available.notes && <p className="whitespace-pre-line text-caption text-muted">{available.notes}</p>}
          <button type="button" disabled={busy || !canInstall} onClick={update.install} className={buttonClass}>
            {status === 'installing' ? 'Instalando…' : 'Atualizar e reiniciar'}
          </button>
          {!canInstall && status !== 'installing' && (
            <p className="text-caption text-muted">Dá para atualizar fora da fila, da seleção e da partida.</p>
          )}
        </>
      ) : (
        <button type="button" disabled={busy} onClick={update.check} className={buttonClass}>
          {status === 'checking' ? 'Procurando…' : 'Procurar atualização'}
        </button>
      )}
      {status === 'upToDate' && <p className="text-caption text-muted">Você está na versão mais recente.</p>}
      {status === 'checkFailed' && <p role="alert" className="text-caption text-danger">Não deu para procurar atualização agora.</p>}
      {status === 'installFailed' && <p role="alert" className="text-caption text-danger">Não deu para instalar a atualização. Tente de novo mais tarde.</p>}
    </section>
  );
}

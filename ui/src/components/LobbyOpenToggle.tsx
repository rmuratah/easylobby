import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { Lobby } from '../bindings/Lobby';
import { cx } from './base/cx';

type Props = { lobby: Lobby; send: AgentClient['send']; disabled?: boolean };

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';

/**
 * Botão de alternância do lobby aberto ou fechado (#225): com o lobby aberto,
 * amigos entram sem convite. Só o líder troca; para os outros fica desabilitado
 * mostrando o estado. O estado exibido é o que o agente confirmou.
 */
export function LobbyOpenToggle({ lobby, send, disabled = false }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const open = lobby.isOpen;
  const locked = disabled || !lobby.isLeader || pending;
  const hint = lobby.isLeader
    ? open ? 'Amigos entram sem convite. Clique para fechar.' : 'Só entra quem for convidado. Clique para abrir.'
    : 'Só o líder abre ou fecha o lobby.';
  return (
    <span className="inline-flex flex-col items-end gap-1">
      <button type="button" role="switch" aria-checked={open} aria-busy={pending} disabled={locked} title={hint}
        onClick={() => { void run({ type: 'setLobbyOpen', open: !open }, { pending: open ? 'Fechando o lobby…' : 'Abrindo o lobby…', failure: open ? 'fechar o lobby' : 'abrir o lobby' }); }}
        className={cx('inline-flex h-10 items-center gap-2 rounded-full surface px-3 text-caption font-semibold shadow-soft transition-colors disabled:cursor-not-allowed', focusRing,
          open ? 'text-accent-ink' : 'text-muted')}>
        <span aria-hidden="true" className={cx('relative h-5 w-9 shrink-0 rounded-full transition-colors', open ? 'bg-accent' : 'bg-raised ring-1 ring-control')}>
          <span className={cx('absolute top-0.5 size-4 rounded-full bg-surface shadow-soft transition-all', open ? 'left-[1.125rem]' : 'left-0.5')} />
        </span>
        Lobby {open ? 'aberto' : 'fechado'}
      </button>
      {feedback.status === 'error' && <span role="alert" className="max-w-64 text-right text-caption text-danger">{feedback.message}</span>}
    </span>
  );
}

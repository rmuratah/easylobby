import { useEffect, useId, useRef, useState } from 'react';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import { cx } from './base/cx';

type Props = {
  /** Nome do jogador, para os textos do menu e da confirmação. */
  name: string;
  summonerId: number;
  send: AgentClient['send'];
  /** Na fila, na partida encontrada ou fora da fase de lobby, o menu fica desabilitado. */
  disabled?: boolean;
  /**
   * Para que lado o menu abre (#303): `end` (padrão) alinha pela direita do
   * botão e abre para a esquerda; `start` alinha pela esquerda do botão e abre
   * para a direita, para caber nos cards estreitos da Arena.
   */
  align?: 'start' | 'end';
  /**
   * Se o jogador pode convidar (#412), o `allowedInviteOthers` do membro.
   * Com o valor, o menu ganha o item que libera ou tira a permissão; sem ele
   * (personalizada), o item não aparece.
   */
  allowedInvite?: boolean;
};

type Action = 'promote' | 'kick';

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const itemClass = `w-full rounded-control px-3 py-2 text-left text-body font-medium transition-colors hover:bg-raised ${focusRing}`;

const texts: Record<Action, { label: string; question: (name: string) => string; confirm: string; pending: string; failure: string }> = {
  promote: {
    label: 'Tornar líder',
    question: name => `Passar a liderança para ${name}? Só o novo líder pode devolver.`,
    confirm: 'Tornar líder',
    pending: 'Passando a liderança…',
    failure: 'passar a liderança',
  },
  kick: {
    label: 'Remover do grupo',
    question: name => `Remover ${name} do grupo?`,
    confirm: 'Remover',
    pending: 'Removendo do grupo…',
    failure: 'remover do grupo',
  },
};

/**
 * Menu "⋯" no card de outro jogador do lobby (#267): o líder passa a
 * liderança ou remove o jogador do grupo, as duas com confirmação, e libera
 * ou tira a permissão de convidar (#412). Quem chama
 * só mostra o menu para o líder e fora do próprio card. O lobby novo chega
 * pelo agente; o menu não guarda estado do lobby.
 */
export function LobbyMemberMenu({ name, summonerId, send, disabled = false, align = 'end', allowedInvite }: Props) {
  const side = align === 'end' ? 'right-0' : 'left-0';
  const { feedback, pending, run } = useCommand(send);
  const [open, setOpen] = useState(false);
  const [confirming, setConfirming] = useState<Action | null>(null);
  const menuId = useId();
  const root = useRef<HTMLDivElement>(null);
  const locked = disabled || pending;

  function close() {
    setOpen(false);
    setConfirming(null);
  }

  // Fecha com Esc e com clique fora.
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === 'Escape') close(); };
    const onPointer = (event: PointerEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) close();
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer);
    };
  }, [open]);

  // Entrar na fila com o menu aberto fecha o menu.
  if (open && disabled) close();

  // Liberar ou tirar o convite não pede confirmação: dá para desfazer pelo mesmo menu (#412).
  async function toggleInvite() {
    close();
    const allowed = !allowedInvite;
    await run({ type: 'setLobbyMemberInvite', summonerId, allowed },
      { pending: allowed ? 'Liberando convites…' : 'Tirando a permissão de convite…', failure: allowed ? 'liberar os convites' : 'tirar a permissão de convite' });
  }

  async function confirm(action: Action) {
    close();
    const { pending: pendingText, failure } = texts[action];
    await run(action === 'promote'
      ? { type: 'promoteLobbyMember', summonerId }
      : { type: 'kickLobbyMember', summonerId }, { pending: pendingText, failure });
  }

  return (
    <div ref={root} className="relative shrink-0 self-start">
      <button type="button" aria-label={`Ações para ${name}`} title={disabled ? 'Disponível na fase de lobby' : `Ações para ${name}`}
        aria-haspopup="menu" aria-expanded={open} aria-controls={open ? menuId : undefined} aria-busy={pending} disabled={locked}
        onClick={() => { if (open) close(); else setOpen(true); }}
        className={cx('flex size-8 items-center justify-center rounded-full text-muted transition-colors enabled:hover:bg-raised enabled:hover:text-ink disabled:cursor-not-allowed disabled:opacity-50', focusRing)}>
        <span aria-hidden="true" className="text-section leading-none">⋯</span>
      </button>
      {open && (
        <div id={menuId} className={cx('absolute top-9 z-20 w-64 rounded-tile bg-surface p-2 shadow-lift ring-1 ring-line', side)}>
          {confirming ? (
            <div role="alertdialog" aria-label={texts[confirming].label} className="flex flex-col gap-2 p-1">
              <p className="text-body">{texts[confirming].question(name)}</p>
              <div className="flex justify-end gap-2">
                <button type="button" onClick={() => setConfirming(null)}
                  className={`rounded-full px-3 py-1.5 text-caption font-semibold text-muted hover:bg-raised ${focusRing}`}>
                  Cancelar
                </button>
                <button type="button" onClick={() => { void confirm(confirming); }}
                  className={cx('rounded-full px-3 py-1.5 text-caption font-semibold', focusRing,
                    confirming === 'kick' ? 'bg-raised text-danger ring-1 ring-danger hover:bg-surface' : 'bg-accent text-on-accent')}>
                  {texts[confirming].confirm}
                </button>
              </div>
            </div>
          ) : (
            <ul role="menu" aria-label={`Ações para ${name}`} className="flex flex-col">
              {allowedInvite !== undefined && (
                <li role="none">
                  <button type="button" role="menuitem" onClick={() => { void toggleInvite(); }} className={itemClass}>
                    {allowedInvite ? 'Tirar permissão de convite' : 'Permitir convites'}
                  </button>
                </li>
              )}
              {(['promote', 'kick'] as const).map(action => (
                <li key={action} role="none">
                  <button type="button" role="menuitem" onClick={() => setConfirming(action)}
                    className={cx(itemClass, action === 'kick' && 'text-danger')}>
                    {texts[action].label}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
      {feedback.status === 'pending' && <span role="status" className="sr-only">{feedback.message}</span>}
      {feedback.status === 'error' && (
        <p role="alert" className={cx('absolute top-9 z-10 w-64 rounded-tile bg-surface p-2 text-caption text-danger shadow-soft', side)}>{feedback.message}</p>
      )}
    </div>
  );
}

import { useEffect, useRef, useState, type ReactNode } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { Command } from '../bindings/Command';
import { Button, EmptyState, Spinner } from './base';
import { honoredPlayers, playerName, votesLeft } from '../agent/honor';
import { HonorVote } from './HonorVote';
import { GameResultView } from './GameResultView';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /** Abre o perfil de um jogador do placar (#585). */
  onOpenProfile?: (puuid: string) => void;
  /**
   * `false` enquanto outra página (o perfil aberto pelo placar) cobre o
   * pós-jogo: o painel fica montado, sem perder a honra já votada, mas o Enter
   * não joga de novo.
   */
  visible?: boolean;
};

/**
 * Quanto esperar as estatísticas em `WaitingForStats` antes de avisar que
 * elas não chegaram (#184). No LoL real o bloco costuma chegar em segundos.
 */
export const STATS_TIMEOUT_MS = 60_000;

/** Comando do "Sair": recusa o Jogar de novo do grupo e volta ao início (#180). */
const LEAVE: Command = { type: 'leavePostGame' };

/**
 * Pós-jogo. Primeiro a votação de honra (#182), enquanto a cédula estiver
 * aberta, sobrarem honras e o jogador não tiver pulado; depois o resultado e
 * o placar da partida (#179, #183), quando o agente já mandou, com "Jogar de
 * novo" (o mesmo grupo e a mesma fila, `playAgain`, #180) e "Sair" (#184).
 *
 * Sem resultado, mostra a espera em `WaitingForStats` e um aviso quando a
 * partida termina com erro (`TerminatedInError`) ou as estatísticas não
 * chegam, sempre com uma saída.
 */
export function PostGamePanel({ connection, send, onOpenProfile, visible = true }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const [doneGameId, setDoneGameId] = useState<number | null>(null);
  const [running, setRunning] = useState<'playAgain' | 'leave' | null>(null);
  const available = connection.status === 'connected' && connection.state?.lcu.state === 'connected';
  const phase = connection.state?.gameflowPhase;
  const ballot = connection.state?.honorBallot ?? null;
  const result = connection.state?.gameResult ?? null;
  const crashed = phase === 'TerminatedInError';
  const waiting = phase === 'WaitingForStats' && !result;
  const late = useElapsed(waiting, STATS_TIMEOUT_MS);
  const voting = !crashed && ballot !== null && doneGameId !== ballot.gameId && votesLeft(ballot) > 0;
  const honored = ballot ? honoredPlayers(ballot) : [];

  async function act(kind: 'playAgain' | 'leave') {
    setRunning(kind);
    if (kind === 'playAgain') await run({ type: 'playAgain' }, { pending: 'Voltando ao lobby…', failure: 'jogar de novo' });
    else await run(LEAVE, { pending: 'Saindo…', failure: 'sair do fim de partida' });
    setRunning(null);
  }

  // Enter joga de novo, como o Enter que aceita a partida (#129), a não ser
  // que o foco esteja num campo ou botão, que já tratam o Enter.
  const shortcut = visible && !voting && !crashed && available && !pending;
  const playAgainRef = useRef(act);
  useEffect(() => { playAgainRef.current = act; });
  useEffect(() => {
    if (!shortcut) return;
    function onKey(event: KeyboardEvent) {
      if (event.key !== 'Enter' || event.repeat || event.defaultPrevented) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest('input, textarea, select, button, a, [contenteditable="true"]')) return;
      event.preventDefault();
      void playAgainRef.current('playAgain');
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [shortcut]);

  if (ballot && voting) {
    return (
      <section aria-label="Fim de partida" className="flex flex-1 flex-col items-center justify-center rounded-panel surface card shadow-soft animate-fade-up px-6 py-10">
        <HonorVote ballot={ballot} send={send} disabled={!available} onDone={() => { setDoneGameId(ballot.gameId); }} />
      </section>
    );
  }
  const honorStatus = ballot && !crashed && <p role="status" className="text-body text-muted">
    {honored.length > 0 ? `Você honrou ${list(honored.map(playerName))}.`
      : ballot.votes === 0 ? 'Você não tem honras nesta partida.' : 'Você pulou a honra desta partida.'}</p>;

  const playAgainButton = (variant: 'primary' | 'secondary') => (
    <Button variant={variant} onClick={() => { void act('playAgain'); }} aria-keyshortcuts={crashed ? undefined : 'Enter'}
      disabled={!available || pending} loading={running === 'playAgain'}>
      {running === 'playAgain' ? 'Voltando ao lobby…' : 'Jogar de novo'}
    </Button>
  );
  const leaveButton = (variant: 'primary' | 'secondary', label: string) => (
    <Button variant={variant} onClick={() => { void act('leave'); }} disabled={!available || pending} loading={running === 'leave'}>
      {running === 'leave' ? 'Saindo…' : label}
    </Button>
  );
  const actions = (buttons: ReactNode, hint: boolean) => (
    <div className="flex flex-col items-center gap-2">
      <div className="flex flex-wrap items-center justify-center gap-3">{buttons}</div>
      {hint && shortcut && <span className="text-caption text-muted">Enter joga de novo</span>}
      {feedback.status === 'error' && <p role="alert" className="max-w-[22rem] text-caption text-danger">{feedback.message}</p>}
    </div>
  );
  const usual = actions(<>{playAgainButton('primary')}{leaveButton('secondary', 'Sair')}</>, true);

  if (connection.state && result) {
    return (
      <section aria-label="Fim de partida" className="flex flex-1 flex-col gap-4 overflow-y-auto rounded-panel surface card p-5 shadow-soft animate-fade-up">
        <GameResultView result={result} state={connection.state} actions={usual} onOpenProfile={onOpenProfile} />
        {honorStatus && <div className="flex justify-center">{honorStatus}</div>}
      </section>
    );
  }

  let state: ReactNode;
  if (crashed) {
    state = <EmptyState title="A partida terminou com erro" icon={<span className="text-[1.75rem] font-semibold text-danger">!</span>}
      description={<span role="alert">O LoL não mandou o resultado desta partida. Volte ao início para continuar.</span>}
      action={actions(<>{leaveButton('primary', 'Voltar ao início')}{playAgainButton('secondary')}</>, false)} />;
  } else if (waiting && !late) {
    state = <EmptyState title="Partida encerrada" icon={<Spinner size={28} label={null} />}
      description={<span role="status">Calculando o resultado…</span>}
      action={<div className="flex flex-col items-center gap-4">{honorStatus}{usual}</div>} />;
  } else {
    state = <EmptyState title="O resultado não chegou"
      description="O LoL não mandou as estatísticas desta partida. Jogue de novo com o mesmo grupo ou volte ao início."
      action={<div className="flex flex-col items-center gap-4">{honorStatus}
        {actions(<>{playAgainButton('primary')}{leaveButton('secondary', 'Voltar ao início')}</>, true)}</div>} />;
  }
  return (
    <section aria-label="Fim de partida" className="flex flex-1 flex-col items-center justify-center rounded-panel surface card shadow-soft animate-fade-up">
      {state}
    </section>
  );
}

/** `true` depois de `active` ficar ligado por `ms` sem desligar. */
function useElapsed(active: boolean, ms: number): boolean {
  const [elapsed, setElapsed] = useState(false);
  useEffect(() => {
    if (!active) return;
    const timer = setTimeout(() => setElapsed(true), ms);
    return () => { clearTimeout(timer); setElapsed(false); };
  }, [active, ms]);
  return active && elapsed;
}

/** "A", "A e B", "A, B e C". */
function list(names: string[]): string {
  return names.length <= 1 ? names.join('') : `${names.slice(0, -1).join(', ')} e ${names.at(-1)}`;
}

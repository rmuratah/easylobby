import { useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import { Button, EmptyState } from './base';
import { honoredPlayers, playerName, votesLeft } from '../agent/honor';
import { HonorVote } from './HonorVote';
import { GameResultView } from './GameResultView';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

/**
 * Pós-jogo. Primeiro a votação de honra (#182), enquanto a cédula estiver
 * aberta, sobrarem honras e o jogador não tiver pulado; depois o resultado e
 * o placar da partida (#179, #183), quando o agente já mandou, e a volta ao
 * lobby com o mesmo grupo e a mesma fila (`playAgain`, #180).
 */
export function PostGamePanel({ connection, send }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const [doneGameId, setDoneGameId] = useState<number | null>(null);
  const available = connection.status === 'connected' && connection.state?.lcu.state === 'connected';
  const waiting = connection.state?.gameflowPhase === 'WaitingForStats';
  const ballot = connection.state?.honorBallot ?? null;
  const result = connection.state?.gameResult ?? null;
  const voting = ballot !== null && doneGameId !== ballot.gameId && votesLeft(ballot) > 0;
  const honored = ballot ? honoredPlayers(ballot) : [];

  if (ballot && voting) {
    return (
      <section aria-label="Fim de partida" className="flex flex-1 flex-col items-center justify-center rounded-panel surface card shadow-soft animate-fade-up px-6 py-10">
        <HonorVote ballot={ballot} send={send} disabled={!available} onDone={() => { setDoneGameId(ballot.gameId); }} />
      </section>
    );
  }
  const honorStatus = ballot && <p role="status" className="text-body text-muted">
    {honored.length > 0 ? `Você honrou ${list(honored.map(playerName))}.`
      : ballot.votes === 0 ? 'Você não tem honras nesta partida.' : 'Você pulou a honra desta partida.'}</p>;
  const playAgain = (
    <div className="flex flex-col items-center gap-2">
      <Button variant="primary" onClick={() => { void run({ type: 'playAgain' }, { pending: 'Voltando ao lobby…', failure: 'voltar ao lobby' }); }}
        disabled={!available} loading={pending}>
        {pending ? 'Voltando ao lobby…' : 'Voltar ao lobby'}
      </Button>
      {feedback.status === 'error' && <p role="alert" className="max-w-[22rem] text-caption text-danger">{feedback.message}</p>}
    </div>
  );
  if (connection.state && result) {
    return (
      <section aria-label="Fim de partida" className="flex flex-1 flex-col gap-4 overflow-y-auto rounded-panel surface card p-5 shadow-soft animate-fade-up">
        <GameResultView result={result} state={connection.state} actions={playAgain} />
        {honorStatus && <div className="flex justify-center">{honorStatus}</div>}
      </section>
    );
  }
  return (
    <section aria-label="Fim de partida" className="flex flex-1 flex-col items-center justify-center rounded-panel surface card shadow-soft animate-fade-up">
      <EmptyState title="Partida encerrada"
        description={waiting ? 'Calculando o resultado da partida.' : 'Volte ao lobby para jogar de novo com o mesmo grupo.'}
        action={<div className="flex flex-col items-center gap-4">
          {honorStatus}
          {playAgain}
        </div>} />
    </section>
  );
}

/** "A", "A e B", "A, B e C". */
function list(names: string[]): string {
  return names.length <= 1 ? names.join('') : `${names.slice(0, -1).join(', ')} e ${names.at(-1)}`;
}

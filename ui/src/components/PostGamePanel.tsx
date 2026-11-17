import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import { Button, EmptyState } from './base';
import { HonorVote } from './HonorVote';
import { GameResultView } from './GameResultView';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

/**
 * Tela inicial do pós-jogo: a votação de honra enquanto a cédula estiver
 * aberta (#182) e a volta ao lobby da partida, com o mesmo grupo e a mesma
 * fila (`playAgain`, #180). Com o resultado da partida (#179), mostra o
 * resultado e o placar (#183).
 */
export function PostGamePanel({ connection, send }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const available = connection.status === 'connected' && connection.state?.lcu.state === 'connected';
  const waiting = connection.state?.gameflowPhase === 'WaitingForStats';
  const ballot = connection.state?.honorBallot ?? null;
  const result = connection.state?.gameResult ?? null;
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
        {ballot && <div className="flex justify-center"><HonorVote ballot={ballot} send={send} disabled={!available} /></div>}
        <GameResultView result={result} state={connection.state} actions={playAgain} />
      </section>
    );
  }
  return (
    <section aria-label="Fim de partida" className="flex flex-1 flex-col items-center justify-center rounded-panel surface card shadow-soft animate-fade-up">
      <EmptyState title="Partida encerrada"
        description={waiting ? 'Calculando o resultado da partida.' : 'Volte ao lobby para jogar de novo com o mesmo grupo.'}
        action={<div className="flex flex-col items-center gap-4">
          {ballot && <HonorVote ballot={ballot} send={send} disabled={!available} />}
          {playAgain}
        </div>} />
    </section>
  );
}

import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import { Button, EmptyState } from './base';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

/**
 * Tela inicial do pós-jogo: por enquanto só volta ao lobby da partida, com o
 * mesmo grupo e a mesma fila (`playAgain`, #180). Resultado, placar e honra
 * entram depois (#183, #182).
 */
export function PostGamePanel({ connection, send }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const available = connection.status === 'connected' && connection.state?.lcu.state === 'connected';
  const waiting = connection.state?.gameflowPhase === 'WaitingForStats';
  return (
    <section aria-label="Fim de partida" className="flex flex-1 flex-col items-center justify-center rounded-panel surface shadow-soft animate-fade-up">
      <EmptyState title="Partida encerrada"
        description={waiting ? 'Calculando o resultado da partida.' : 'Volte ao lobby para jogar de novo com o mesmo grupo.'}
        action={<div className="flex flex-col items-center gap-2">
          <Button onClick={() => { void run({ type: 'playAgain' }, { pending: 'Voltando ao lobby…', failure: 'voltar ao lobby' }); }}
            disabled={!available} loading={pending}>
            {pending ? 'Voltando ao lobby…' : 'Voltar ao lobby'}
          </Button>
          {feedback.status === 'error' && <p role="alert" className="max-w-[22rem] text-caption text-danger">{feedback.message}</p>}
        </div>} />
    </section>
  );
}

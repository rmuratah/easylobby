import { useEffect, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import { Button } from './base';
import { cx } from './base/cx';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type QueueCommand = Extract<Command, { type: 'startQueue' | 'stopQueue' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };

function duration(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  return `${minutes}:${String(total % 60).padStart(2, '0')}`;
}

function stateLabel(state: AgentState['matchmaking']): string {
  switch (state?.searchState) {
    case 'Searching': return 'Buscando partida';
    case 'Found': return 'Partida encontrada';
    case 'Error': return 'Busca interrompida por erro';
    case 'AbandonedLowPriorityQueue': return 'Fila de baixa prioridade abandonada';
    case 'Unknown': return 'Estado da busca desconhecido';
    default: return 'Fora da fila';
  }
}

function QueueTime({ confirmed, running }: { confirmed: number; running: boolean }) {
  const [extra, setExtra] = useState(0);
  useEffect(() => {
    if (!running) return;
    const startedAt = Date.now();
    const timer = setInterval(() => setExtra((Date.now() - startedAt) / 1000), 1000);
    return () => clearInterval(timer);
  }, [running]);
  return <strong aria-label="Tempo na fila">{duration(confirmed + extra)}</strong>;
}

export function QueuePanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <section aria-label="Busca de partida" className="w-full max-w-lg rounded border border-slate-700 p-4">
        <h2 className="text-xl font-semibold">Busca de partida</h2>
        <p>Aguarde a reconexão. O estado da fila será confirmado novamente pelo agente.</p>
      </section>
    );
  }
  return <QueueActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} />;
}

function QueueActions({ state, send }: { state: AgentState; send: AgentClient['send'] }) {
  const { matchmaking, lobby, summoner, gameflowPhase: phase } = state;
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const inFlight = useRef(false);
  const mounted = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  const pending = feedback.status === 'pending';
  const canStart = !state.readyCheck && !!summoner && !!lobby && lobby.isLeader && lobby.canStartActivity && phase === 'Lobby' && !matchmaking;
  const canStop = !state.readyCheck && !!matchmaking && phase === 'Matchmaking';
  const restriction = !summoner ? 'Aguarde o login no League of Legends.'
    : matchmaking ? (canStop ? 'A busca confirmada pelo agente está em andamento.' : 'Aguarde a próxima fase confirmada pelo agente.')
    : !lobby ? 'Crie ou entre em um lobby antes de buscar uma partida.'
    : !lobby.isLeader ? 'Somente o líder do lobby pode iniciar a busca.'
    : phase !== 'Lobby' ? 'A busca só pode ser iniciada na fase de lobby.'
    : !lobby.canStartActivity ? 'O lobby ainda não está pronto para iniciar a busca.'
    : 'O lobby está pronto para buscar uma partida.';

  async function execute(command: QueueCommand) {
    if (inFlight.current || (command.type === 'startQueue' ? !canStart : !canStop)) return;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: command.type === 'startQueue' ? 'Iniciando busca…' : 'Cancelando busca…' });
    try {
      const result = await send(command);
      if (!mounted.current) return;
      if (result.status === 'ok') {
        setFeedback({ status: 'success', message: command.type === 'startQueue'
          ? 'Comando aceito. O estado exibido é o confirmado pelo agente.'
          : 'Cancelamento confirmado pelo agente.' });
      } else {
        setFeedback({ status: 'error', message: result.code === 'notConnected'
          ? 'O League of Legends desconectou. Aguarde a reconexão e confira a fila antes de tentar novamente.'
          : `Não foi possível ${command.type === 'startQueue' ? 'iniciar' : 'cancelar'} a busca: ${result.message}` });
      }
    } catch {
      if (!mounted.current) return;
      setFeedback({ status: 'error', message: 'Não foi possível confirmar o comando. Confira a conexão e o estado da fila antes de tentar novamente.' });
    } finally {
      inFlight.current = false;
    }
  }

  const searching = matchmaking?.searchState === 'Searching';
  return (
    <section aria-label="Busca de partida"
      className={cx('sticky bottom-0 z-10 flex w-full flex-col gap-3 rounded-card border p-4 text-ink shadow-[0_-8px_24px_-12px_var(--el-overlay)]',
        searching ? 'border-accent bg-accent-soft' : 'border-line bg-surface')}>
      <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <h2 className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">Busca de partida</h2>
          <p className="text-section">Estado: <strong className="font-semibold">{stateLabel(matchmaking)}</strong></p>
          {matchmaking && (
            <p className="flex flex-wrap gap-x-4 text-body text-muted">
              <span>Tempo na fila: <span className="text-section font-semibold tabular-nums text-ink">
                <QueueTime key={`${matchmaking.searchState}:${matchmaking.timeInQueue}`}
                  confirmed={matchmaking.timeInQueue} running={matchmaking.searchState === 'Searching'} /></span></span>
              <span>Estimativa: <strong className="font-semibold tabular-nums text-ink">{matchmaking.estimatedQueueTime > 0 ? duration(matchmaking.estimatedQueueTime) : 'indisponível'}</strong></span>
            </p>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant={canStop ? 'secondary' : 'primary'} size="lg" aria-busy={pending}
            disabled={!canStart || pending} onClick={() => { void execute({ type: 'startQueue' }); }}>Iniciar busca</Button>
          <Button variant={canStop ? 'primary' : 'secondary'} size="lg" aria-busy={pending}
            disabled={!canStop || pending} onClick={() => { void execute({ type: 'stopQueue' }); }}>Cancelar busca</Button>
        </div>
      </div>
      {matchmaking && matchmaking.lowPriorityTimeRemaining > 0 && (
        <p role="alert" className="text-body text-warn">Baixa prioridade: {duration(matchmaking.lowPriorityTimeRemaining)} restantes.</p>
      )}
      {matchmaking && matchmaking.errors.length > 0 && (
        <ul aria-label="Erros da busca" className="text-body text-warn">
          {matchmaking.errors.map((error, index) => (
            <li key={`${error.errorType}:${index}`}>
              {error.message || error.errorType || 'Erro de matchmaking'}
              {error.penaltyTimeRemaining > 0 ? ` · ${duration(error.penaltyTimeRemaining)} restantes` : ''}
            </li>
          ))}
        </ul>
      )}
      <p className="text-caption text-muted">{restriction}</p>
      {feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}
        className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>}
    </section>
  );
}

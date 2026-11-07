import { useEffect, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
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
      <section aria-label="Busca de partida" className="flex w-full flex-col gap-2 rounded-card border border-line bg-surface p-5 text-ink">
        <h2 className="text-section font-semibold">Busca de partida</h2>
        <p className="text-body text-muted">Aguarde a reconexão. O estado da fila será confirmado novamente pelo agente.</p>
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
  const startRef = useRef<HTMLButtonElement>(null);
  const stopRef = useRef<HTMLButtonElement>(null);
  // Com a busca confirmada, o botão principal vira "Cancelar busca" (e volta ao terminar).
  const active = !!matchmaking;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  // Quem estava no botão que sumiu continua no botão que tomou o lugar dele.
  useEffect(() => {
    const hidden = active ? startRef.current : stopRef.current;
    if (hidden && document.activeElement === hidden) (active ? stopRef.current : startRef.current)?.focus();
  }, [active]);
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
  const big = 'h-14 min-w-0 flex-1 rounded-full px-6 text-section font-semibold tracking-wide transition-all disabled:cursor-not-allowed focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
  return (
    <section aria-label="Busca de partida"
      className={cx('flex h-full w-full flex-col gap-3 rounded-panel surface p-4 text-ink shadow-lift animate-fade-up @min-[44rem]:shadow-soft',
        searching && 'ring-2 ring-accent/30')}>
      <div className="flex flex-col gap-1 px-1">
        <h2 className="text-section font-semibold">Busca de partida</h2>
        <p className="text-body text-muted">Estado: <strong className="font-semibold text-ink">{stateLabel(matchmaking)}</strong></p>
      </div>
      {matchmaking && (
        <div className="flex flex-wrap items-end justify-between gap-x-4 gap-y-2 rounded-tile bg-raised px-4 py-3">
          <div className="flex flex-col">
            <span className="text-caption text-muted">Tempo na fila</span>
            <span className="text-title font-semibold tabular-nums text-ink">
              <QueueTime key={`${matchmaking.searchState}:${matchmaking.timeInQueue}`}
                confirmed={matchmaking.timeInQueue} running={matchmaking.searchState === 'Searching'} />
            </span>
          </div>
          <div className="flex flex-col items-end">
            <span className="text-caption text-muted">Estimativa</span>
            <strong className="text-section font-semibold tabular-nums text-ink">{matchmaking.estimatedQueueTime > 0 ? duration(matchmaking.estimatedQueueTime) : 'indisponível'}</strong>
          </div>
        </div>
      )}
      {matchmaking && matchmaking.lowPriorityTimeRemaining > 0 && (
        <p role="alert" className="px-1 text-body text-warn">Baixa prioridade: {duration(matchmaking.lowPriorityTimeRemaining)} restantes.</p>
      )}
      {matchmaking && matchmaking.errors.length > 0 && (
        <ul aria-label="Erros da busca" className="px-1 text-body text-warn">
          {matchmaking.errors.map((error, index) => (
            <li key={`${error.errorType}:${index}`}>
              {error.message || error.errorType || 'Erro de matchmaking'}
              {error.penaltyTimeRemaining > 0 ? ` · ${duration(error.penaltyTimeRemaining)} restantes` : ''}
            </li>
          ))}
        </ul>
      )}
      <p className="rounded-tile bg-gold-soft px-4 py-2 text-center text-caption font-medium text-warn">{restriction}</p>
      {feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}
        className={cx('px-1', feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted')}>{feedback.message}</p>}
      <div className="mt-auto flex items-center gap-2">
        <button ref={startRef} type="button" hidden={active} aria-busy={pending}
          disabled={!canStart || pending} onClick={() => { void execute({ type: 'startQueue' }); }}
          className={cx(big, 'bg-accent text-on-accent shadow-soft enabled:hover:-translate-y-0.5 disabled:bg-raised disabled:text-muted disabled:shadow-none')}>
          Iniciar busca
        </button>
        <button ref={stopRef} type="button" hidden={!active} aria-busy={pending}
          disabled={!canStop || pending} onClick={() => { void execute({ type: 'stopQueue' }); }}
          className={cx(big, 'bg-surface text-accent-ink ring-2 ring-accent disabled:text-muted disabled:ring-control',
            searching && canStop && !pending && 'animate-search-ring')}>
          Cancelar busca
        </button>
      </div>
    </section>
  );
}

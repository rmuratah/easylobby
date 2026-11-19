import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { sound } from '../agent/sound';
import { hoverSound } from '../agent/buttonSounds';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { AgentState } from '../bindings/AgentState';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { Command } from '../bindings/Command';
import { duration, lowPriorityText, matchmakingErrorText, queuePenalty } from '../agent/matchmaking';
import { awayMembers, pendingChoices, restrictionReasons } from '../agent/lobbyRestrictions';
import { memberName } from '../agent/friends';
import { cx } from './base/cx';
import { useDismiss } from '../agent/useDismiss';
import { lobbyQueueName } from '../agent/queueOptions';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /**
   * Lugar do botão Jogar, na barra do alto (#600). Com ele, o botão da busca sai do card e
   * toma o lugar do Jogar; o card fica só com o estado, o tempo e os motivos.
   */
  buttonSlot?: HTMLElement | null;
};
const READY_IN_SLOT = 'O lobby está pronto. Use Buscar partida, no alto da janela.';
type QueueCommand = Extract<Command, { type: 'startQueue' | 'stopQueue' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };

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

function QueueTime({ confirmed, running, labelled = true }: { confirmed: number; running: boolean; labelled?: boolean }) {
  const [extra, setExtra] = useState(0);
  useEffect(() => {
    if (!running) return;
    const startedAt = Date.now();
    const timer = setInterval(() => setExtra((Date.now() - startedAt) / 1000), 1000);
    return () => clearInterval(timer);
  }, [running]);
  return labelled ? <strong aria-label="Tempo na fila">{duration(confirmed + extra)}</strong> : <>{duration(confirmed + extra)}</>;
}

export function QueuePanel({ connection, send, buttonSlot = null }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <section aria-label="Busca de partida" className="flex w-full flex-col gap-2 rounded-card border border-edge bg-surface card p-5 text-ink">
        <h2 className="text-section font-semibold">Busca de partida</h2>
        <p className="text-body text-muted">Aguarde a reconexão com o League of Legends.</p>
      </section>
    );
  }
  return <QueueActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} buttonSlot={buttonSlot} />;
}

function QueueActions({ state, send, buttonSlot }: { state: AgentState; send: AgentClient['send']; buttonSlot: HTMLElement | null }) {
  const { matchmaking, lobby, summoner, gameflowPhase: phase } = state;
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  // O erro some sozinho depois de alguns segundos (#335).
  useDismiss(feedback.status === 'error', feedback.message, () => setFeedback({ status: 'idle', message: '' }));
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
  // Na personalizada (Modo de Treino, #229) a partida começa pelo lobby, sem busca.
  const custom = !!lobby?.custom;
  // Escolhas obrigatórias que faltam (#334): a busca só libera com todas feitas.
  const nameOf = (member: LobbyMember) => memberName(member, state.friends);
  const missing = pendingChoices(lobby, summoner?.puuid, nameOf);
  // Quem ainda está na tela de fim de jogo (#346): a busca espera todos voltarem.
  // O card do jogador no lobby já marca quem não voltou: aqui fica só o aviso geral.
  const away = awayMembers(lobby, phase);
  // O LoL trava a fila enquanto baixa um patch (#575).
  const patching = !!state.lolUpdate?.blocksQueue;
  const canStart = !state.readyCheck && !!summoner && !!lobby && !custom && lobby.isLeader && lobby.canStartActivity && phase === 'Lobby' && !matchmaking
    && missing.length === 0 && away.length === 0 && !patching;
  // A busca parada por punição também sai, mesmo se a fase não for `Matchmaking`.
  const canStop = !state.readyCheck && !!matchmaking && (phase === 'Matchmaking' || queuePenalty(matchmaking));
  // Motivos da LCU para a fila bloqueada (#227), no lugar do aviso genérico.
  const reasons = away.length ? [] : [...restrictionReasons(lobby, summoner?.puuid, nameOf), ...missing];
  // Com a busca em andamento, o card já mostra o estado e o tempo na fila: sem aviso extra.
  const restriction = !summoner ? 'Aguarde o login no League of Legends.'
    : matchmaking ? ''
    : !lobby ? 'Crie ou entre em um lobby antes de buscar uma partida.'
    : custom ? 'Na personalizada, a partida começa pelo botão Iniciar partida, no lobby.'
    : patching ? 'O League of Legends está se atualizando. A busca libera sozinha quando o patch terminar.'
    : !lobby.isLeader ? 'Somente o líder do lobby pode iniciar a busca.'
    : phase !== 'Lobby' ? 'A busca só pode ser iniciada na fase de lobby.'
    : away.length ? 'Aguarde todos voltarem ao lobby.'
    : !lobby.canStartActivity ? (reasons.length ? 'A busca não pode começar:' : 'O lobby ainda não está pronto para iniciar a busca.')
    : missing.length ? 'Faltam escolhas antes de buscar:'
    : buttonSlot ? READY_IN_SLOT
    : 'O lobby está pronto para buscar uma partida.';

  async function execute(command: QueueCommand) {
    if (inFlight.current || (command.type === 'startQueue' ? !canStart : !canStop)) return;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: command.type === 'startQueue' ? 'Iniciando busca…' : 'Cancelando busca…' });
    try {
      const result = await send(command);
      if (!mounted.current) return;
      if (result.status === 'ok') {
        // O próprio card mostra o resultado (estado e botão mudam): sem mensagem de sucesso.
        setFeedback({ status: 'idle', message: '' });
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
  const start = () => { sound.play('click'); void execute({ type: 'startQueue' }); };
  const stop = () => { void execute({ type: 'stopQueue' }); };
  // No alto da janela (#600): o mesmo par de botões, com o desenho do Jogar.
  const topButtons = buttonSlot && createPortal(<>
    <button ref={startRef} type="button" hidden={active} aria-busy={pending} aria-label="Buscar partida"
      title={canStart ? 'Buscar partida' : restriction || 'Buscar partida'}
      disabled={!canStart || pending} onMouseEnter={hoverSound} onClick={start} className="app-play-button app-queue-button">
      <span className="app-play-button-face">
        <span className="flex min-w-0 flex-col items-center leading-tight">
          <span aria-hidden="true">Buscar partida</span>
          {lobby && <span aria-hidden="true" className="app-play-button-detail">{lobbyQueueName(lobby, state.queues)}</span>}
        </span>
      </span>
    </button>
    <button ref={stopRef} type="button" hidden={!active} aria-busy={pending} aria-label="Cancelar busca" title="Cancelar busca"
      disabled={!canStop || pending} onClick={stop} className={cx('app-play-button app-queue-button', searching && 'is-searching')}>
      <span className="app-play-button-face">
        <span className="flex min-w-0 flex-col items-center leading-tight">
          <span aria-hidden="true">Cancelar busca</span>
          {/* Tempo na fila e estimativa (#603), no lugar do card de busca. */}
          {matchmaking && <span aria-hidden="true" className="app-play-button-detail tabular-nums">
            <QueueTime key={`${matchmaking.searchState}:${matchmaking.timeInQueue}`} confirmed={matchmaking.timeInQueue} running={searching} labelled={false} />
            {matchmaking.estimatedQueueTime > 0 && ` · ~${duration(matchmaking.estimatedQueueTime)}`}
          </span>}
        </span>
      </span>
    </button>
  </>, buttonSlot);
  // Com o botão no alto (#603), o card some: o estado e o tempo ficam no botão e, para o leitor
  // de tela, aqui. Só aparece uma faixa quando há aviso (fila bloqueada, punição, erro).
  const compact = !!buttonSlot;
  const readyNotice = restriction === READY_IN_SLOT;
  const notice = (!!restriction && !readyNotice) || !!feedback.message
    || (!!matchmaking && (matchmaking.lowPriorityTimeRemaining > 0 || matchmaking.errors.length > 0));
  return (
    <section aria-label="Busca de partida"
      className={compact
        ? (notice ? 'flex w-full flex-col gap-2 rounded-panel surface card p-3 text-ink shadow-soft animate-fade-up' : 'sr-only')
        : cx('flex w-full flex-col gap-3 rounded-panel surface card p-4 text-ink shadow-lift animate-fade-up @min-[44rem]:shadow-soft',
          searching && 'ring-2 ring-accent/30')}>
      <div className={compact ? 'sr-only' : 'flex flex-col gap-1 px-1'}>
        <h2 className="text-section font-semibold">Busca de partida</h2>
        <p className="text-body text-muted">Estado: <strong className="font-semibold text-ink">{stateLabel(matchmaking)}</strong></p>
      </div>
      {matchmaking && (
        <div className={compact ? 'sr-only' : 'flex flex-wrap items-end justify-between gap-x-4 gap-y-2 rounded-tile bg-raised px-4 py-3'}>
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
        <p role="alert" className="px-1 text-body text-warn">{lowPriorityText(matchmaking.lowPriorityTimeRemaining)}</p>
      )}
      {matchmaking && matchmaking.errors.length > 0 && (
        <ul aria-label="Erros da busca" className="px-1 text-body text-warn">
          {matchmaking.errors.map((error, index) => (
            <li key={`${error.errorType}:${index}`}>
              {matchmakingErrorText(error)}
            </li>
          ))}
        </ul>
      )}
      {restriction && <div className={compact && readyNotice ? 'sr-only' : 'rounded-tile bg-emerald-soft px-4 py-2 text-center text-caption font-medium text-warn'}>
        <p>{restriction}</p>
        {!matchmaking && !custom && reasons.length > 0 && (
          <ul aria-label="Motivos da fila bloqueada" className="mt-1 flex flex-col gap-0.5 font-semibold">
            {reasons.map(reason => <li key={reason}>{reason}</li>)}
          </ul>
        )}
      </div>}
      {feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}
        className={cx('px-1', feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted')}>{feedback.message}</p>}
      {topButtons || <div className="mt-auto flex items-center gap-2">
        <button ref={startRef} type="button" hidden={active} aria-busy={pending}
          disabled={!canStart || pending} onMouseEnter={hoverSound} onClick={start}
          className={cx(big, 'bg-accent text-on-accent shadow-soft enabled:hover:-translate-y-0.5 disabled:bg-raised disabled:text-muted disabled:shadow-none')}>
          Buscar partida
        </button>
        <button ref={stopRef} type="button" hidden={!active} aria-busy={pending}
          disabled={!canStop || pending} onClick={stop}
          className={cx(big, 'bg-surface text-accent-ink ring-2 ring-accent disabled:text-muted disabled:ring-control',
            searching && canStop && !pending && 'animate-search-ring')}>
          Cancelar busca
        </button>
      </div>}
    </section>
  );
}

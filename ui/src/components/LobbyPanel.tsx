import { useEffect, useId, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { isQueue, queueNames } from '../agent/queues';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import { isPosition, positionNames, validPositions } from '../agent/positions';
import type { Position } from '../bindings/Position';
import type { Queue } from '../bindings/Queue';
import { assetUrl } from '../agent/assets';
import { Avatar } from './base';
import { cx } from './base/cx';
import { CloseIcon, CrownIcon, PlusIcon } from './home/icons';
import { PositionIcon } from './PositionIcon';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type LobbyCommand = Extract<Command, { type: 'createLobby' | 'leaveLobby' | 'setPositions' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const selectClass = `h-10 rounded-tile border border-control bg-raised px-3 text-body text-ink transition-colors disabled:cursor-not-allowed disabled:text-muted ${focusRing}`;
/** Botão em pílula do layout de referência: o principal leva o destaque com brilho; o secundário fica na superfície. */
const pillButton = `inline-flex h-11 shrink-0 items-center justify-center gap-2 rounded-full px-5 text-body font-semibold transition-all disabled:cursor-not-allowed ${focusRing}`;
const primaryPill = `${pillButton} bg-accent text-on-accent shadow-glow enabled:hover:-translate-y-0.5 disabled:bg-raised disabled:text-muted disabled:shadow-none`;

export function LobbyPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <section aria-label="Lobby" className="flex w-full flex-col gap-2 rounded-card border border-line bg-surface p-5 text-ink">
        <h2 className="text-section font-semibold">Lobby</h2>
        <p className="text-body text-muted">Aguarde a conexão com o agente e o League of Legends. Confira o lobby após reconectar antes de tentar novamente.</p>
      </section>
    );
  }
  // Trocar a sessão ou o jogador descarta feedback e respostas da sessão anterior.
  return <LobbyActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} />;
}

/** Espera depois da última escolha antes de enviar, para trocas rápidas (ex.: setas do teclado) virarem um comando só. */
export const AUTO_SAVE_DELAY_MS = 400;
type Outcome = 'skipped' | 'ok' | 'error' | 'stale';
type Desired = { positions?: { first: Position; second: Position }; queue?: Queue };

/** Cargos prontos para salvar: só falta esperar quando há um cargo primário de verdade sem o secundário. */
function positionsComplete(first: Position, second: Position) {
  return second !== 'unselected' || first === 'fill' || first === 'unselected';
}

function LobbyActions({ state, send }: { state: AgentState; send: AgentClient['send'] }) {
  const { lobby, matchmaking, summoner, gameflowPhase: phase } = state;
  const localMember = summoner?.puuid ? lobby?.members.find(member => member.puuid === summoner.puuid) : undefined;
  const first = localMember?.firstPosition ?? 'unselected';
  const second = localMember?.secondPosition ?? 'unselected';
  const contextKey = `${lobby?.queueId}:${lobby?.showPositionSelector}:${localMember?.puuid}`;
  const confirmedKey = `${contextKey}:${first}:${second}`;
  // Escolha ainda não confirmada pelo agente (agendada ou em envio): um evento do lobby não a apaga da tela.
  const [syncing, setSyncing] = useState({ positions: false, queue: false });
  const [positions, setPositions] = useState({ contextKey, confirmedKey, first, second });
  const [acceptedPositions, setAcceptedPositions] = useState<{ confirmedKey: string; first: Position; second: Position } | null>(null);
  if (positions.confirmedKey !== confirmedKey) {
    const keep = syncing.positions && positions.contextKey === contextKey;
    setPositions(keep ? { ...positions, confirmedKey } : { contextKey, confirmedKey, first, second });
    if (!keep && syncing.positions) setSyncing({ ...syncing, positions: false });
    setAcceptedPositions(null);
  }
  const queueId = lobby?.queueId ?? null;
  const [selection, setSelection] = useState<{ queueId: number | null; queue: Queue }>({ queueId, queue: lobby?.queue ?? 'normalDraft' });
  if (selection.queueId !== queueId) {
    setSelection({ queueId, queue: syncing.queue && lobby ? selection.queue : lobby?.queue ?? 'normalDraft' });
  }
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const desired = useRef<Desired>({});
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const descriptionId = useId();
  const selectId = useId();
  const firstId = useId();
  const secondId = useId();
  const positionContext = useRef(0);
  const confirmedRevision = useRef(0);
  useEffect(() => { confirmedRevision.current += 1; }, [confirmedKey]);
  useEffect(() => {
    positionContext.current += 1;
    // Cargos escolhidos para outra fila ou outro membro não valem mais.
    delete desired.current.positions;
  }, [queueId, lobby?.showPositionSelector, localMember?.puuid]);
  const inLobby = !!lobby;
  useEffect(() => {
    if (!inLobby) delete desired.current.queue;
  }, [inLobby]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      clearTimeout(timer.current);
    };
  }, []);

  const phaseAllowsChange = !state.readyCheck && !matchmaking && (phase === 'None' || phase === 'Lobby');
  const canCreate = !!summoner && phaseAllowsChange && (!lobby || lobby.isLeader);
  const canLeave = !state.readyCheck && !!summoner && !!lobby && !matchmaking && phase === 'Lobby';
  const canSetPositions = !state.readyCheck && !!localMember && !!lobby?.showPositionSelector && !matchmaking && phase === 'Lobby';
  // Cargos que o LoL tem (ou acabou de aceitar, antes do evento chegar).
  const saved = acceptedPositions?.confirmedKey === confirmedKey ? acceptedPositions : { first, second };
  const pending = feedback.status === 'pending';
  const localName = summoner?.gameName
    ? `${summoner.gameName}${summoner.tagLine ? `#${summoner.tagLine}` : ''}`
    : summoner?.displayName || 'Você';
  const restriction = !summoner ? 'Aguarde o login no League of Legends.'
    : !phaseAllowsChange ? 'Criar, trocar fila e sair do lobby só estão disponíveis fora da busca e da partida. Aguarde a fase de lobby.'
    : lobby && !lobby.isLeader ? 'Somente o líder pode trocar a fila. Você pode sair do lobby.'
    : lobby ? 'A fila muda assim que você escolhe outra.'
    : 'Escolha uma fila para criar um lobby.';

  async function execute(command: LobbyCommand): Promise<Outcome> {
    if (inFlight.current || (command.type === 'createLobby'
      ? !canCreate || command.queue === lobby?.queue
      : command.type === 'leaveLobby' ? !canLeave
      : !canSetPositions || !validPositions(command.first, command.second)
        || (command.first === saved.first && command.second === saved.second))) return 'skipped';
    const context = positionContext.current;
    const revision = confirmedRevision.current;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: command.type === 'setPositions' ? 'Salvando cargos…' : command.type === 'leaveLobby' ? 'Saindo do lobby…'
      : lobby ? 'Trocando fila…' : 'Criando lobby…' });
    try {
      const result = await send(command);
      if (!mounted.current) return 'stale';
      if (command.type === 'setPositions' && context !== positionContext.current) {
        setFeedback({ status: 'idle', message: '' });
        return 'stale';
      }
      if (result.status === 'ok') {
        // Evita repetir um comando aceito enquanto o evento ainda não chegou.
        if (command.type === 'setPositions' && revision === confirmedRevision.current) {
          setAcceptedPositions({ confirmedKey, first: command.first, second: command.second });
        }
        setFeedback({ status: 'success', message: command.type === 'setPositions'
          ? 'Cargos salvos.' : command.type === 'leaveLobby'
          ? 'Saída do lobby confirmada.' : 'Lobby atualizado pelo agente.' });
        return 'ok';
      }
      setFeedback({ status: 'error', message: result.code === 'notConnected'
        ? 'O League of Legends desconectou. Aguarde a reconexão e confira o lobby antes de tentar novamente.'
        : `Não foi possível alterar o lobby: ${result.message}` });
      return 'error';
    } catch {
      if (!mounted.current) return 'stale';
      setFeedback({ status: 'error', message: 'Não foi possível confirmar o comando. Confira a conexão e o lobby antes de tentar novamente.' });
      return 'error';
    } finally {
      inFlight.current = false;
      // Uma escolha feita durante o envio sai logo depois, sem esperar outra mudança.
      if (mounted.current && (desired.current.positions || desired.current.queue)) schedule(0);
    }
  }
  const executeRef = useRef(execute);
  useEffect(() => { executeRef.current = execute; });

  function schedule(delay = AUTO_SAVE_DELAY_MS) {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => { void flush(); }, delay);
  }

  /** Envia a última escolha guardada, uma por vez; se o LoL recusar, a tela volta ao que o agente confirmou. */
  async function flush() {
    timer.current = undefined;
    if (inFlight.current) return;
    const next = desired.current;
    const kind = next.queue ? 'queue' : next.positions ? 'positions' : null;
    if (!kind) return;
    let command: LobbyCommand;
    if (kind === 'queue') {
      command = { type: 'createLobby', queue: next.queue! };
      delete next.queue;
    } else {
      command = { type: 'setPositions', ...next.positions! };
      delete next.positions;
    }
    const outcome = await executeRef.current(command);
    if (!mounted.current || desired.current[kind]) return;
    setSyncing(current => ({ ...current, [kind]: false }));
    if (outcome === 'ok') return;
    // Recusado, falhou ou não pôde mais ser enviado: força a tela a reler o valor confirmado.
    // Cargos iguais aos já salvos também caem aqui ('skipped'), e aí a tela já mostra o valor certo.
    if (kind === 'queue') setSelection(current => ({ ...current, queueId: -1 }));
    else if (outcome === 'error') setPositions(current => ({ ...current, confirmedKey: '' }));
  }

  function choosePositions(next: { first: Position; second: Position }) {
    setPositions({ ...positions, ...next });
    if (positionsComplete(next.first, next.second)) {
      desired.current.positions = next;
      setSyncing(current => ({ ...current, positions: true }));
      schedule();
    } else {
      // Falta o cargo secundário: nada é enviado até ele ser escolhido.
      delete desired.current.positions;
    }
  }

  const searching = !!matchmaking || !!state.readyCheck;
  const feedbackLine = feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}
    className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>;
  const queueSelect = (className: string) => (
    <select
      id={selectId}
      value={selection.queue}
      disabled={!canCreate || (!lobby && pending)}
      aria-describedby={descriptionId}
      onChange={event => {
        if (!isQueue(event.target.value)) return;
        const queue = event.target.value;
        setSelection({ queueId, queue });
        // Sem lobby, criar continua sendo um clique em "Criar lobby"; com lobby, a fila troca sozinha.
        if (!lobby || !canCreate) return;
        desired.current.queue = queue;
        setSyncing(current => ({ ...current, queue: true }));
        schedule();
      }}
      className={className}
    >
      {(Object.keys(queueNames) as Queue[]).map(queue => <option key={queue} value={queue}>{queueNames[queue]}</option>)}
    </select>
  );

  // Sem lobby, o painel vira o cartão principal do início: chamada grande, fila e "Criar lobby".
  if (!lobby) {
    return (
      <section aria-label="Lobby"
        className="relative isolate flex h-full min-h-[24rem] overflow-hidden rounded-panel bg-surface text-ink shadow-lift animate-fade-up">
        <div aria-hidden="true" className="absolute inset-0 -z-10 bg-hero-art" />
        <div aria-hidden="true" className="pointer-events-none absolute -right-20 top-1/2 -z-10 size-[26rem] -translate-y-1/2 rounded-full border border-accent/25">
          <div className="absolute inset-12 rounded-full border border-accent/30" />
          <div className="absolute inset-28 rounded-full bg-primary-gradient opacity-30 blur-2xl" />
        </div>
        <div aria-hidden="true" className="absolute inset-0 -z-10 bg-hero-fade" />
        <div className="flex max-w-md flex-col justify-end gap-5 p-8 @min-[44rem]:p-10">
          <span className="w-fit rounded-full glass px-3 py-1 text-caption font-semibold uppercase tracking-wider text-accent-ink">Jogar</span>
          <div>
            <h2 className="text-display font-semibold">Pronto para jogar?</h2>
            <p className="mt-3 text-section text-muted">Você não está em um lobby.</p>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <label htmlFor={selectId} className="sr-only">Fila</label>
            {queueSelect(`h-11 min-w-0 flex-1 rounded-full glass px-4 text-body font-semibold text-ink disabled:cursor-not-allowed disabled:text-muted ${focusRing}`)}
            <button type="button" className={primaryPill} aria-describedby={descriptionId} aria-busy={pending}
              disabled={!canCreate || pending}
              onClick={() => { void execute({ type: 'createLobby', queue: selection.queue }); }}>Criar lobby</button>
          </div>
          <p id={descriptionId} className="text-caption text-muted">{restriction}</p>
          {feedbackLine}
        </div>
      </section>
    );
  }

  const queueLabel = lobby.queue ? queueNames[lobby.queue] : `Outra fila (ID ${lobby.queueId})`;
  const freeSlots = Math.max(0, lobby.maxLobbySize - lobby.members.length);
  return (
    <section aria-label="Lobby" className="flex h-full min-h-0 w-full flex-col gap-4 text-ink">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">Lobby</h2>
          <p className="text-title font-semibold"><span className="sr-only">Fila atual: </span><strong className="font-semibold">{queueLabel}</strong></p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span className="rounded-full bg-accent-soft px-3 py-1.5 text-caption font-semibold text-accent-ink">
            {lobby.members.length} / {lobby.maxLobbySize} jogadores
          </span>
          <span className="rounded-full surface px-3 py-1.5 shadow-soft text-caption font-medium text-muted">
            {lobby.isLeader ? 'Você é o líder' : 'Você é membro'}
          </span>
          {/* Na fila e na partida encontrada, a faixa de ação fica com a busca; sair volta depois. */}
          {!searching && (
            <button type="button" aria-label="Sair do lobby" title="Sair do lobby" aria-describedby={descriptionId} aria-busy={pending}
              disabled={!canLeave || pending} onClick={() => { void execute({ type: 'leaveLobby' }); }}
              className={`flex size-10 items-center justify-center rounded-tile surface text-muted shadow-soft transition-colors enabled:hover:text-danger disabled:cursor-not-allowed ${focusRing}`}>
              <CloseIcon />
            </button>
          )}
        </div>
      </header>

      <ul aria-label="Jogadores no lobby" className="flex min-h-0 flex-1 flex-col gap-3">
        {lobby.members.map((member, index) => {
          const me = member.puuid === summoner?.puuid;
          const name = me ? localName : `Invocador ${member.summonerId}`;
          return (
            <li key={member.puuid} style={{ animationDelay: `${index * 70}ms` }}
              className={cx('relative flex min-h-20 flex-1 items-center gap-4 rounded-panel px-4 py-3 transition-all duration-200 animate-fade-up',
                me ? 'bg-surface shadow-lift ring-2 ring-accent/30' : 'surface shadow-soft hover:-translate-y-0.5 hover:shadow-lift')}>
              <div className={cx('shrink-0 rounded-full p-1', me ? 'bg-primary-gradient' : 'bg-raised')}>
                <Avatar name={me && summoner?.gameName ? summoner.gameName : name} src={assetUrl('profileIcon', member.summonerIconId)}
                  size={me ? 64 : 56} className="ring-2 ring-surface" />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <h3 className={cx('truncate font-semibold', me ? 'text-section' : 'text-body')}>{name}</h3>
                  {member.isLeader && (
                    <span role="img" aria-label="Líder do grupo" title="Líder do grupo"
                      className="flex size-6 shrink-0 items-center justify-center rounded-full bg-gold-soft text-warn">
                      <CrownIcon size={14} />
                    </span>
                  )}
                </div>
                {me && <p className="truncate text-caption text-muted">Você</p>}
                <span className="mt-2 inline-block rounded-full bg-raised px-2.5 py-1 text-caption font-semibold text-muted">Nível {member.summonerLevel}</span>
              </div>
              {lobby.showPositionSelector && (
                <ul aria-label={`Cargos de ${name}`} className="flex w-36 shrink-0 flex-col gap-2 @min-[52rem]:w-52">
                  {[member.firstPosition, member.secondPosition].map((position, slot) => (
                    <li key={slot} className={cx('flex items-center gap-2 rounded-tile px-3 py-2 text-body font-medium',
                      slot === 0 ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-muted')}>
                      <PositionIcon position={position} />
                      <span className="truncate">{positionNames[position]}</span>{' '}
                      {/* Em janela estreita, a ordem e a cor dizem qual é qual; o leitor de tela continua ouvindo. */}
                      <span className="sr-only shrink-0 text-caption @min-[52rem]:not-sr-only @min-[52rem]:ml-auto">{slot === 0 ? 'Primário' : 'Secundário'}</span>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          );
        })}
        {Array.from({ length: freeSlots }, (_, index) => (
          <li key={`vaga-${index}`} aria-label="Vaga livre" style={{ animationDelay: `${(lobby.members.length + index) * 70}ms` }}
            className="flex min-h-20 flex-1 items-center justify-center gap-3 rounded-panel border-2 border-dashed border-line bg-surface/40 text-muted animate-fade-up">
            <span aria-hidden="true" className="flex size-11 items-center justify-center rounded-full bg-raised"><PlusIcon size={20} /></span>
            <span className="text-body font-semibold">Vaga livre</span>
          </li>
        ))}
      </ul>

      <div className={cx('grid items-stretch gap-4', !searching && '@min-[44rem]:grid-cols-2')}>
        {lobby.showPositionSelector ? (
          <fieldset className="flex h-full min-w-0 flex-col gap-3 rounded-panel surface p-4 shadow-soft animate-fade-up" disabled={!canSetPositions}>
            <legend className="sr-only">Seus cargos</legend>
            <h3 aria-hidden="true" className="px-1 text-section font-semibold">Seus cargos</h3>
            <p className="px-1 text-caption text-muted">As posições na lista de jogadores são confirmadas pelo agente.</p>
            {!localMember && <p className="px-1 text-body text-muted">Aguarde a identificação do seu membro no lobby.</p>}
            {phase !== 'Lobby' && <p className="px-1 text-body text-muted">A escolha de cargos está disponível na fase de lobby.</p>}
            <div className="grid grid-cols-2 gap-2">
              <label className="flex min-w-0 flex-col gap-1.5 px-1 text-caption font-medium text-muted" htmlFor={firstId}>Cargo primário
                <span className="relative flex items-center">
                  <PositionIcon position={positions.first} className="pointer-events-none absolute left-3 z-10" />
                <select id={firstId} value={positions.first} className={`${selectClass} w-full pl-10`}
                  onChange={event => {
                    if (!isPosition(event.target.value)) return;
                    const next = event.target.value;
                    choosePositions({ first: next, second: validPositions(next, positions.second) ? positions.second : 'unselected' });
                  }}>
                  {(Object.keys(positionNames) as Position[]).map(position =>
                    <option key={position} value={position}>{positionNames[position]}</option>)}
                </select></span>
              </label>
              <label className="flex min-w-0 flex-col gap-1.5 px-1 text-caption font-medium text-muted" htmlFor={secondId}>Cargo secundário
                <span className="relative flex items-center">
                  <PositionIcon position={positions.second} className="pointer-events-none absolute left-3 z-10" />
                <select id={secondId} value={positions.second} className={`${selectClass} w-full pl-10`}
                  disabled={!canSetPositions || positions.first === 'fill' || positions.first === 'unselected'}
                  onChange={event => {
                    if (isPosition(event.target.value)) choosePositions({ first: positions.first, second: event.target.value });
                  }}>
                  {(Object.keys(positionNames) as Position[]).map(position =>
                    <option key={position} value={position} disabled={!validPositions(positions.first, position)}>{positionNames[position]}</option>)}
                </select></span>
              </label>
            </div>
            {!positionsComplete(positions.first, positions.second) && (
              <p className="px-1 text-caption text-muted">Escolha o cargo secundário para salvar.</p>
            )}
          </fieldset>
        ) : <p className="rounded-panel surface p-4 shadow-soft text-body text-muted animate-fade-up">Esta fila não permite escolher cargos.</p>}
        {/* Na fila e na partida encontrada, a faixa de ação fica com a busca; trocar fila volta depois. A troca é feita ao escolher. */}
        {!searching && (
          <div className="flex h-full min-w-0 flex-col gap-3 rounded-panel surface p-4 shadow-soft animate-fade-up" style={{ animationDelay: '70ms' }}>
            <label className="px-1 text-section font-semibold" htmlFor={selectId}>Fila</label>
            {queueSelect(selectClass)}
            <p id={descriptionId} className="px-1 text-caption text-muted">{restriction}</p>
          </div>
        )}
      </div>
      {feedbackLine}
    </section>
  );
}

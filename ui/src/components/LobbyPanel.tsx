import { useCallback, useEffect, useId, useRef, useState, type DragEvent } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { lobbyQueueName, queueKey, type QueueKey } from '../agent/queueOptions';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import { memberName } from '../agent/friends';
import { FRIEND_DRAG_TYPE, useLobbyInvite } from '../agent/lobbyInvite';
import { InvitePicker } from './LobbyInvites';
import { isPosition, positionNames, validPositions } from '../agent/positions';
import { queuePenalty } from '../agent/matchmaking';
import type { Position } from '../bindings/Position';
import { assetUrl } from '../agent/assets';
import { Avatar } from './base';
import { cx } from './base/cx';
import { CloseIcon, CrownIcon, PlusIcon } from './home/icons';
import { PositionIcon } from './PositionIcon';
import { ModeSelect } from './ModeSelect';
import { QuickPlaySlots } from './QuickPlaySlots';
import { LobbyRank } from './LobbyRank';
import { LobbyOpenToggle } from './LobbyOpenToggle';
import { LobbyLinkButton } from './LobbyLinkButton';
import { LobbyMemberMenu } from './LobbyMemberMenu';
import { PracticeLobby } from './PracticeLobby';
import { ArenaTeams } from './ArenaTeams';
import { isTutorialQueue } from '../agent/training';
import { useDismiss } from '../agent/useDismiss';
import { awayMembers } from '../agent/lobbyRestrictions';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /** Abre o perfil de outro jogador ao clicar no card dele (#308). */
  onOpenProfile?: (puuid: string) => void;
};
type LobbyCommand = Extract<Command, { type: 'createLobby' | 'createLobbyById' | 'leaveLobby' | 'setPositions' | 'startTutorial' | 'startPracticeTool' | 'createCustomGame' | 'joinCustomGame' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const selectClass = `h-10 rounded-tile border border-control bg-raised px-3 text-body text-ink transition-colors disabled:cursor-not-allowed disabled:text-muted ${focusRing}`;
/** Botão em pílula do layout de referência: o principal leva o destaque com brilho; o secundário fica na superfície. */

export function LobbyPanel({ connection, send, onOpenProfile }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <section aria-label="Lobby" className="flex w-full flex-col gap-2 rounded-card border border-edge bg-surface card p-5 text-ink">
        <h2 className="text-section font-semibold">Lobby</h2>
        <p className="text-body text-muted">Aguarde a conexão com o agente e o League of Legends. Confira o lobby após reconectar antes de tentar novamente.</p>
      </section>
    );
  }
  // Trocar a sessão ou o jogador descarta feedback e respostas da sessão anterior.
  return <LobbyActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} onOpenProfile={onOpenProfile} />;
}

/** Espera depois da última escolha antes de enviar, para trocas rápidas (ex.: setas do teclado) virarem um comando só. */
export const AUTO_SAVE_DELAY_MS = 400;

function commandKey(command: Extract<Command, { type: 'createLobby' | 'createLobbyById' }>): QueueKey {
  return command.type === 'createLobby' ? command.queue : `fila-${command.queueId}`;
}
type Outcome = 'skipped' | 'ok' | 'error' | 'stale';
type Desired = { positions?: { first: Position; second: Position } };

/** Posições prontas para salvar: só falta esperar quando há uma posição primária de verdade sem a secundária. */
function positionsComplete(first: Position, second: Position) {
  return second !== 'unselected' || first === 'fill' || first === 'unselected';
}

function LobbyActions({ state, send, onOpenProfile }: { state: AgentState; send: AgentClient['send']; onOpenProfile?: (puuid: string) => void }) {
  const { lobby, matchmaking, summoner, gameflowPhase: phase } = state;
  const localMember = summoner?.puuid ? lobby?.members.find(member => member.puuid === summoner.puuid) : undefined;
  // Quem ainda está na tela de fim de jogo (#346): o card fica esmaecido, como no client oficial.
  const away = new Set(awayMembers(lobby, phase).map(member => member.puuid));
  const first = localMember?.firstPosition ?? 'unselected';
  const second = localMember?.secondPosition ?? 'unselected';
  const contextKey = `${lobby?.queueId}:${lobby?.showPositionSelector}:${localMember?.puuid}`;
  const confirmedKey = `${contextKey}:${first}:${second}`;
  // Escolha ainda não confirmada pelo agente (agendada ou em envio): um evento do lobby não a apaga da tela.
  const [syncing, setSyncing] = useState({ positions: false });
  const [positions, setPositions] = useState({ contextKey, confirmedKey, first, second });
  const [acceptedPositions, setAcceptedPositions] = useState<{ confirmedKey: string; first: Position; second: Position } | null>(null);
  if (positions.confirmedKey !== confirmedKey) {
    const keep = syncing.positions && positions.contextKey === contextKey;
    setPositions(keep ? { ...positions, confirmedKey } : { contextKey, confirmedKey, first, second });
    if (!keep && syncing.positions) setSyncing({ ...syncing, positions: false });
    setAcceptedPositions(null);
  }
  const queueId = lobby?.queueId ?? null;
  const currentKey: QueueKey | null = lobby ? queueKey(lobby) : null;
  // Tela de escolha de modo aberta por cima do lobby para trocar a fila (#231).
  const [changingMode, setChangingMode] = useState(false);
  const invites = useLobbyInvite(state, send);
  const [picker, setPicker] = useState<number | null>(null);
  // Avisos dos slots do Jogo Dinâmico (#218), no quadro abaixo da lista.
  const [slotNotes, setSlotNotes] = useState<HTMLDivElement | null>(null);
  const [dropping, setDropping] = useState(false);
  const closePicker = useCallback(() => setPicker(null), []);
  // Os recentes mudam a cada partida; o pop-up pede de novo ao abrir (#206). Um erro deixa a lista anterior.
  const loadRecent = useCallback(() => { Promise.resolve().then(() => send({ type: 'loadRecentPlayers' })).catch(() => {}); }, [send]);
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  // O erro some sozinho depois de alguns segundos (#335).
  useDismiss(feedback.status === 'error', feedback.message, () => setFeedback({ status: 'idle', message: '' }));
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const desired = useRef<Desired>({});
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const descriptionId = useId();
  const firstId = useId();
  const secondId = useId();
  const positionContext = useRef(0);
  const confirmedRevision = useRef(0);
  useEffect(() => { confirmedRevision.current += 1; }, [confirmedKey]);
  useEffect(() => {
    positionContext.current += 1;
    // Posições escolhidas para outra fila ou outro membro não valem mais.
    delete desired.current.positions;
  }, [queueId, lobby?.showPositionSelector, localMember?.puuid]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      clearTimeout(timer.current);
    };
  }, []);

  const phaseAllowsChange = !state.readyCheck && !matchmaking && (phase === 'None' || phase === 'Lobby');
  const canCreate = !!summoner && phaseAllowsChange && (!lobby || lobby.isLeader);
  // Com a busca parada por punição (dodge, abandono), dá para sair do lobby: o agente sai da busca antes.
  const penalty = queuePenalty(matchmaking);
  const canLeave = !state.readyCheck && !!summoner && !!lobby
    && (matchmaking ? penalty && (phase === 'Lobby' || phase === 'Matchmaking') : phase === 'Lobby');
  const canSetPositions = !state.readyCheck && !!localMember && !!lobby?.showPositionSelector && !matchmaking && phase === 'Lobby';
  // Tornar líder e remover do grupo (#267): fora da fila e da partida encontrada.
  const canManageMembers = !state.readyCheck && !matchmaking && phase === 'Lobby';
  // Jogo Dinâmico (#218): posições e campeões escolhidos no próprio card.
  const canSetSlots = !state.readyCheck && !!localMember && !!lobby?.showQuickPlaySlots && !matchmaking && phase === 'Lobby';
  // Posições que o LoL tem (ou acabou de aceitar, antes do evento chegar).
  const saved = acceptedPositions?.confirmedKey === confirmedKey ? acceptedPositions : { first, second };
  const pending = feedback.status === 'pending';
  const localName = summoner?.gameName
    ? `${summoner.gameName}${summoner.tagLine ? `#${summoner.tagLine}` : ''}`
    : summoner?.displayName || 'Você';
  const restriction = !summoner ? 'Aguarde o login no League of Legends.'
    : !phaseAllowsChange ? 'Criar, trocar fila e sair do lobby só estão disponíveis fora da busca e da partida. Aguarde a fase de lobby.'
    : lobby && !lobby.isLeader ? 'Somente o líder pode trocar a fila. Você pode sair do lobby.'
    : lobby ? 'Para trocar a fila, use Trocar modo.'
    : 'Escolha um modo para criar um lobby.';

  async function execute(command: LobbyCommand): Promise<Outcome> {
    if (inFlight.current || (command.type === 'createLobby' || command.type === 'createLobbyById'
      ? !canCreate || commandKey(command) === currentKey
      : command.type === 'startTutorial' || command.type === 'startPracticeTool' || command.type === 'createCustomGame' || command.type === 'joinCustomGame' ? !canCreate || !!lobby
      : command.type === 'leaveLobby' ? !canLeave
      : !canSetPositions || !validPositions(command.first, command.second)
        || (command.first === saved.first && command.second === saved.second))) return 'skipped';
    const context = positionContext.current;
    const revision = confirmedRevision.current;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: command.type === 'setPositions' ? 'Salvando posições…' : command.type === 'leaveLobby' ? 'Saindo do lobby…'
      : command.type === 'startTutorial' ? 'Iniciando o Tutorial…' : command.type === 'startPracticeTool' ? 'Iniciando o Modo de Treino…'
      : command.type === 'createCustomGame' ? 'Criando a partida personalizada…'
      : command.type === 'joinCustomGame' ? 'Entrando na partida personalizada…'
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
          ? 'Posições salvas.' : command.type === 'leaveLobby'
          ? 'Saída do lobby confirmada.' : command.type === 'startTutorial' ? 'Tutorial iniciado.'
          : command.type === 'startPracticeTool' ? 'Modo de Treino iniciado.'
          : command.type === 'createCustomGame' ? 'Partida personalizada criada.'
          : command.type === 'joinCustomGame' ? 'Você entrou na partida personalizada.' : 'Lobby atualizado pelo agente.' });
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
      if (mounted.current && desired.current.positions) schedule(0);
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
    const next = desired.current.positions;
    if (!next) return;
    delete desired.current.positions;
    const outcome = await executeRef.current({ type: 'setPositions', ...next });
    if (!mounted.current || desired.current.positions) return;
    setSyncing({ positions: false });
    // Recusado ou falhou: força a tela a reler o valor confirmado. Posições iguais às já salvas
    // ('skipped') já mostram o valor certo.
    if (outcome === 'error') setPositions(current => ({ ...current, confirmedKey: '' }));
  }

  function choosePositions(next: { first: Position; second: Position }) {
    setPositions({ ...positions, ...next });
    if (positionsComplete(next.first, next.second)) {
      desired.current.positions = next;
      setSyncing({ positions: true });
      schedule();
    } else {
      // Falta a posição secundária: nada é enviado até ela ser escolhida.
      delete desired.current.positions;
    }
  }

  const searching = !!matchmaking || !!state.readyCheck;
  const feedbackLine = feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}
    className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>;
  // Sem lobby, o painel vira a tela de escolha de modo de jogo (#210).
  if (!lobby) {
    return (
      <ModeSelect queues={state.queues} canCreate={canCreate} pending={pending} restriction={restriction}
        descriptionId={descriptionId} feedback={feedbackLine} tutorial={state.tutorial} summonerName={summoner?.gameName || summoner?.displayName}
        onCreate={command => { void execute(command); }} onStart={command => { void execute(command); }}
        onCreateCustom={options => { void execute({ type: 'createCustomGame', options }); }}
        customJoin={{ games: state.customGames, send,
          onJoin: (partyId, password) => { void execute({ type: 'joinCustomGame', partyId, ...(password ? { password } : {}) }); } }} />
    );
  }
  // No lobby, "Trocar modo" abre a mesma tela para trocar a fila (#231); na busca ou na partida, volta ao lobby.
  if (changingMode && !searching) {
    return (
      <ModeSelect queues={state.queues} canCreate={canCreate} pending={pending}
        restriction={canCreate ? 'Escolha outra fila e confirme em Trocar fila.' : restriction}
        descriptionId={descriptionId} feedback={feedbackLine}
        lobby={{ queueId: lobby.queueId, queueName: lobbyQueueName(lobby, state.queues), onBack: () => setChangingMode(false) }}
        onCreate={command => {
          void execute(command).then(outcome => { if (outcome === 'ok' && mounted.current) setChangingMode(false); });
        }}
        onStart={() => {}} />
    );
  }

  // Personalizada, como o Modo de Treino (#229): duas equipes com bots.
  if (lobby.custom) {
    return <PracticeLobby lobby={{ ...lobby, custom: lobby.custom }} state={state} send={send} canLeave={canLeave}
      leaving={pending} onLeave={() => { void execute({ type: 'leaveLobby' }); }} feedback={feedbackLine} />;
  }

  // Amigo arrastado da lista de amigos (#206): solto em qualquer ponto do lobby, vira convite.
  const isFriendDrag = (event: DragEvent) => event.dataTransfer.types.includes(FRIEND_DRAG_TYPE);
  function onDragOver(event: DragEvent) {
    if (!isFriendDrag(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = invites.canInvite ? 'copy' : 'none';
    if (!dropping) setDropping(true);
  }
  function onDragLeave(event: DragEvent) {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDropping(false);
  }
  function onDrop(event: DragEvent) {
    if (!isFriendDrag(event)) return;
    event.preventDefault();
    setDropping(false);
    const puuid = event.dataTransfer.getData(FRIEND_DRAG_TYPE);
    if (puuid) invites.inviteByPuuid(puuid);
  }

  const queueLabel = lobbyQueueName(lobby, state.queues);
  const freeSlots = Math.max(0, lobby.maxLobbySize - lobby.members.length);
  return (
    <section aria-label="Lobby" onDragOver={onDragOver} onDragLeave={onDragLeave} onDrop={onDrop}
      className={cx('flex h-full min-h-0 w-full flex-col gap-4 rounded-panel text-ink transition-shadow',
        dropping && 'ring-2 ring-accent')}>
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
          {/* Lobby aberto ou fechado (#225): todos os modos, menos o Tutorial. */}
          {!isTutorialQueue(lobby.queueId) && <LobbyOpenToggle lobby={lobby} send={send} disabled={phase !== 'Lobby'} />}
          {/* Link para entrar no lobby (#226): todos os modos, menos o Tutorial.
              Também durante a busca (Matchmaking), como no client oficial. */}
          {!isTutorialQueue(lobby.queueId) && <LobbyLinkButton link={state.lobbyLink} send={send}
            disabled={phase !== 'Lobby' && phase !== 'Matchmaking'} />}
          {!searching && (
            <button type="button" aria-describedby={descriptionId} disabled={!canCreate || pending}
              onClick={() => { setFeedback({ status: 'idle', message: '' }); setChangingMode(true); }}
              className={`inline-flex h-10 items-center rounded-full surface px-4 text-body font-semibold shadow-soft transition-colors enabled:hover:text-accent-ink disabled:cursor-not-allowed disabled:text-muted ${focusRing}`}>
              Trocar modo
            </button>
          )}
          {/* Na fila e na partida encontrada, a faixa de ação fica com a busca; sair volta depois (ou já, com punição de fila). */}
          {(!searching || (penalty && !state.readyCheck)) && (
            <button type="button" aria-label="Sair do lobby" title="Sair do lobby" aria-describedby={descriptionId} aria-busy={pending}
              disabled={!canLeave || pending} onClick={() => { void execute({ type: 'leaveLobby' }); }}
              className={`flex size-10 items-center justify-center rounded-tile surface text-muted shadow-soft transition-colors enabled:hover:text-danger disabled:cursor-not-allowed ${focusRing}`}>
              <CloseIcon />
            </button>
          )}
        </div>
      </header>

      {/* Arena (#224): vagas em equipes, com convidar e trocar de vaga em cada vaga livre. */}
      {lobby.subteams ? (
        <ArenaTeams lobby={lobby} state={state} localName={localName} send={send} picker={picker} onPicker={setPicker} onOpenProfile={onOpenProfile}
          canSwap={canManageMembers && !!localMember} canManageMembers={canManageMembers} />
      ) : (
      <ul aria-label="Jogadores no lobby" className="flex min-h-0 flex-1 flex-col gap-3">
        {lobby.members.map((member, index) => {
          const me = member.puuid === summoner?.puuid;
          const name = me ? localName : memberName(member, state.friends);
          const isAway = away.has(member.puuid);
          return (
            <li key={member.puuid} style={{ animationDelay: `${index * 70}ms` }}
              className={cx('relative flex min-h-20 flex-1 items-center gap-4 rounded-panel card px-4 py-3 transition-all duration-200 animate-fade-up',
                // O card do jogador fica por cima dos outros para a escolha de campeão aberta nele não ficar escondida.
                me ? 'z-10 bg-surface shadow-lift ring-2 ring-accent/30' : 'surface shadow-soft hover:-translate-y-0.5 hover:shadow-lift')}>
              {/* O card de outro jogador inteiro abre o perfil dele (#308). O botão fica por baixo dos
                  controles do card (menu do líder, campeões do Jogo Dinâmico), que vêm depois e são posicionados. */}
              {!me && onOpenProfile && (
                <button type="button" aria-label={`Ver perfil de ${name}`} title={`Ver perfil de ${name}`}
                  onClick={() => onOpenProfile(member.puuid)}
                  className={`absolute inset-0 cursor-pointer rounded-panel ${focusRing}`} />
              )}
              <div className={cx('shrink-0 rounded-full p-1 transition-opacity', me ? 'bg-primary-gradient' : 'bg-raised', isAway && 'opacity-40 grayscale')}>
                <Avatar name={me && summoner?.gameName ? summoner.gameName : name} src={assetUrl('profileIcon', member.summonerIconId)}
                  size={me ? 64 : 56} className="ring-2 ring-surface" />
              </div>
              <div className="min-w-0 flex-1">
                <div className={cx('flex items-center gap-2 transition-opacity', isAway && 'opacity-50')}>
                  <h3 className={cx('truncate font-semibold', me ? 'text-section' : 'text-body')}>{name}</h3>
                  {member.isLeader && (
                    <span role="img" aria-label="Líder do grupo" title="Líder do grupo"
                      className="flex size-6 shrink-0 items-center justify-center rounded-full bg-silver-soft text-warn">
                      <CrownIcon size={14} />
                    </span>
                  )}
                </div>
                {me && <p className="truncate text-caption text-muted">Você</p>}
                {isAway && (
                  <p className="mt-1 inline-flex items-center gap-1.5 rounded-full bg-silver-soft px-2.5 py-1 text-caption font-semibold text-warn">
                    <span aria-hidden="true" className="tracking-widest">•••</span>
                    Ainda não voltou ao lobby
                  </p>
                )}
                <div className={cx('flex flex-wrap items-center gap-x-2 transition-opacity', isAway && 'opacity-50')}>
                  <span className="mt-2 inline-block rounded-full bg-raised px-2.5 py-1 text-caption font-semibold text-muted">Nível {member.summonerLevel}</span>
                  <LobbyRank ranked={member.ranked} queueId={lobby.queueId} />
                </div>
              </div>
              {lobby.showQuickPlaySlots && (
                <QuickPlaySlots key={`${lobby.queueId}:${member.puuid}`} member={member} mine={me} canEdit={canSetSlots}
                  state={state} send={send} notes={me ? slotNotes : undefined} />
              )}
              {lobby.showPositionSelector && (
                <ul aria-label={`Posições de ${name}`} className="flex w-36 shrink-0 flex-col gap-2 @min-[52rem]:w-52">
                  {[member.firstPosition, member.secondPosition].map((position, slot) => (
                    <li key={slot} className={cx('flex items-center gap-2 rounded-tile px-3 py-2 text-body font-medium',
                      slot === 0 ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-muted')}>
                      <PositionIcon position={position} />
                      <span className="truncate">{positionNames[position]}</span>{' '}
                      {/* Em janela estreita, a ordem e a cor dizem qual é qual; o leitor de tela continua ouvindo. */}
                      <span className="sr-only shrink-0 text-caption @min-[52rem]:not-sr-only @min-[52rem]:ml-auto">{slot === 0 ? 'Primária' : 'Secundária'}</span>
                    </li>
                  ))}
                </ul>
              )}
              {/* Tornar líder e remover do grupo (#267): só o líder, nos cards dos outros. */}
              {lobby.isLeader && (me
                // No próprio card não há menu, mas o espaço do "⋯" fica reservado para as posições
                // e os campeões ficarem alinhados com os dos outros cards.
                ? <span aria-hidden="true" data-testid="lobby-menu-spacer" className="size-8 shrink-0 self-start" />
                : <LobbyMemberMenu name={name} summonerId={member.summonerId} send={send} disabled={!canManageMembers} />
              )}
            </li>
          );
        })}
        {Array.from({ length: freeSlots }, (_, index) => (
          <li key={`vaga-${index}`} aria-label="Vaga livre" style={{ animationDelay: `${(lobby.members.length + index) * 70}ms` }}
            className="relative flex min-h-20 flex-1 items-center justify-center gap-3 rounded-panel border-2 border-dashed border-line bg-surface/40 text-muted animate-fade-up">
            {/* O "+" abre a lista de amigos para convidar (#206); sem permissão de convidar, fica só o desenho. */}
            {lobby.canInvite ? (
              <button type="button" aria-label="Convidar amigo" title="Convidar amigo" aria-expanded={picker === index}
                aria-haspopup="dialog" onClick={() => setPicker(picker === index ? null : index)}
                className={`flex size-11 items-center justify-center rounded-full bg-raised text-ink transition-colors hover:bg-accent hover:text-on-accent ${focusRing}`}>
                <PlusIcon size={20} />
              </button>
            ) : <span aria-hidden="true" className="flex size-11 items-center justify-center rounded-full bg-raised"><PlusIcon size={20} /></span>}
            <span className="text-body font-semibold">Vaga livre</span>
            
          </li>
        ))}
      </ul>
      )}
      {invites.error && picker === null && <p role="alert" className="text-body text-danger">{invites.error}</p>}
      {picker !== null && <InvitePicker invites={invites} groups={state.friendGroups} recent={state.recentPlayers}
        loadRecent={loadRecent} onClose={closePicker} />}

      {/* Fila sem posições (ARAM, Arena, Co-op, eventos): nada no lugar, como no client oficial. A fila troca pelo "Trocar modo" (#231). */}
      <div className="grid items-stretch gap-4">
        {lobby.showPositionSelector ? (
          <fieldset className="flex h-full min-w-0 flex-col gap-3 rounded-panel surface card p-4 shadow-soft animate-fade-up" disabled={!canSetPositions}>
            <legend className="sr-only">Suas posições</legend>
            <h3 aria-hidden="true" className="px-1 text-section font-semibold">Suas posições</h3>
            <p className="px-1 text-caption text-muted">As posições na lista de jogadores são confirmadas pelo agente.</p>
            {!localMember && <p className="px-1 text-body text-muted">Aguarde a identificação do seu membro no lobby.</p>}
            {phase !== 'Lobby' && <p className="px-1 text-body text-muted">A escolha de posições está disponível na fase de lobby.</p>}
            <div className="grid grid-cols-2 gap-2">
              <label className="flex min-w-0 flex-col gap-1.5 px-1 text-caption font-medium text-muted" htmlFor={firstId}>Posição primária
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
              <label className="flex min-w-0 flex-col gap-1.5 px-1 text-caption font-medium text-muted" htmlFor={secondId}>Posição secundária
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
              <p className="px-1 text-caption text-muted">Escolha a posição secundária para salvar.</p>
            )}
          </fieldset>
        ) : lobby.showQuickPlaySlots ? (
          <div className="flex h-full min-w-0 flex-col gap-2 rounded-panel surface card p-4 shadow-soft animate-fade-up">
            <h3 className="px-1 text-section font-semibold">Suas posições e campeões</h3>
            <p className="px-1 text-caption text-muted">No Jogo Dinâmico, escolha as duas posições e um campeão para cada uma no seu card, acima.</p>
            <div ref={setSlotNotes} className="flex flex-col gap-1 px-1" />
          </div>
        ) : null}
      </div>
      <p id={descriptionId} className="text-caption text-muted">{restriction}</p>
      {feedbackLine}
    </section>
  );
}

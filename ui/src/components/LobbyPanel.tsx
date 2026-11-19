import { useCallback, useEffect, useId, useRef, useState, type DragEvent } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { lobbyQueueName, queueKey, type QueueKey } from '../agent/queueOptions';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import { memberName } from '../agent/friends';
import { FRIEND_DRAG_TYPE, useLobbyInvite } from '../agent/lobbyInvite';
import { InvitePicker } from './LobbyInvites';
import { positionNames, validPositions } from '../agent/positions';
import { queuePenalty } from '../agent/matchmaking';
import type { Position } from '../bindings/Position';
import { stageSeats } from './lobbyStage';
import { assetUrl } from '../agent/assets';
import { Avatar } from './base';
import { cx } from './base/cx';
import { CloseIcon, CrownIcon, PlusIcon } from './home/icons';
import { PositionIcon } from './PositionIcon';
import { ScarceMark } from './ScarceMark';
import { ModeSelect } from './ModeSelect';
import { QuickPlaySlots } from './QuickPlaySlots';
import { LobbyRank } from './LobbyRank';
import { LobbyOpenToggle } from './LobbyOpenToggle';
import { LobbyLinkButton } from './LobbyLinkButton';
import { LobbyMemberMenu } from './LobbyMemberMenu';
import { PracticeLobby } from './PracticeLobby';
import { ArenaTeams } from './ArenaTeams';
import { MemberPositionOrder, PositionOrderCard } from './PositionOrder';
import { isTutorialQueue } from '../agent/training';
import { useDismiss } from '../agent/useDismiss';
import { awayMembers, blockingMembers } from '../agent/lobbyRestrictions';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /** Abre o perfil de outro jogador ao clicar no card dele (#308). */
  onOpenProfile?: (puuid: string) => void;
  /** Avisa quando a tela de trocar modo abre ou fecha no lobby, para a tela ocupar a área inteira (#513). */
  onModeSelectChange?: (open: boolean) => void;
};
type LobbyCommand = Extract<Command, { type: 'createLobby' | 'createLobbyById' | 'leaveLobby' | 'setPositions' | 'startTutorial' | 'startPracticeTool' | 'createCustomGame' | 'joinCustomGame' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
const CHANGE_MODE_HINT = 'Para trocar a fila, use Trocar modo.';
const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
/** Botão em pílula do layout de referência: o principal leva o destaque com brilho; o secundário fica na superfície. */

export function LobbyPanel({ connection, send, onOpenProfile, onModeSelectChange }: Props) {
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
  return <LobbyActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} onOpenProfile={onOpenProfile} onModeSelectChange={onModeSelectChange} />;
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

function LobbyActions({ state, send, onOpenProfile, onModeSelectChange }: { state: AgentState; send: AgentClient['send'] } & Pick<Props, 'onOpenProfile' | 'onModeSelectChange'>) {
  const { lobby, matchmaking, summoner, gameflowPhase: phase } = state;
  const localMember = summoner?.puuid ? lobby?.members.find(member => member.puuid === summoner.puuid) : undefined;
  // Quem ainda está na tela de fim de jogo (#346): o card fica esmaecido, como no client oficial.
  const away = new Set(awayMembers(lobby, phase).map(member => member.puuid));
  // Quem está bloqueando a fila (#411), com os motivos da LCU: o card ganha um aviso.
  const blocking = blockingMembers(lobby);
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
  // Os sugeridos também (#377); dispensar mostra o erro no pop-up.
  const loadSuggested = useCallback(() => { Promise.resolve().then(() => send({ type: 'loadSuggestedPlayers' })).catch(() => {}); }, [send]);
  const dismissSuggested = useCallback(async (summonerId: number) => {
    try {
      const result = await send({ type: 'dismissSuggestedPlayer', summonerId });
      return result.status === 'ok' ? null : result.message;
    } catch {
      return 'sem resposta do agente';
    }
  }, [send]);
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  // O erro some sozinho depois de alguns segundos (#335).
  useDismiss(feedback.status === 'error', feedback.message, () => setFeedback({ status: 'idle', message: '' }));
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const desired = useRef<Desired>({});
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const descriptionId = useId();
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
    : lobby ? CHANGE_MODE_HINT
    : 'Escolha um modo para criar um lobby.';

  async function execute(command: LobbyCommand): Promise<Outcome> {
    if (inFlight.current || (command.type === 'createLobby' || command.type === 'createLobbyById'
      ? !canCreate || commandKey(command) === currentKey
      : command.type === 'startTutorial' || command.type === 'startPracticeTool' || command.type === 'createCustomGame' || command.type === 'joinCustomGame' ? !canCreate
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
          : command.type === 'joinCustomGame' ? 'Você entrou na partida personalizada.' : 'Lobby atualizado.' });
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
  const modeSelectOpen = !!lobby && changingMode && !searching;
  // Sem lobby (saiu, ou a troca levou para fora dele), a próxima vez abre no lobby, não na troca.
  if (!lobby && changingMode) setChangingMode(false);
  // A tela de troca ocupa a área inteira, sem a busca e o grupo ao lado (#513).
  useEffect(() => {
    onModeSelectChange?.(modeSelectOpen);
  }, [modeSelectOpen, onModeSelectChange]);
  useEffect(() => () => onModeSelectChange?.(false), [onModeSelectChange]);
  const feedbackLine = feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}
    className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>;
  // Sem lobby, o painel vira a tela de escolha de modo de jogo (#210). No lobby,
  // "Trocar modo" abre a mesma tela, com as mesmas abas (#231, #513); na busca ou na partida, volta ao lobby.
  if (!lobby || modeSelectOpen) {
    // Na troca, deu certo: volta ao lobby (o novo, se a escolha saiu do lobby).
    const closeOnSuccess = (outcome: Promise<Outcome>) => {
      void outcome.then(result => { if (result === 'ok' && mounted.current) setChangingMode(false); });
    };
    const run = (command: LobbyCommand) => closeOnSuccess(execute(command));
    return (
      <ModeSelect queues={state.queues} canCreate={canCreate} pending={pending}
        restriction={lobby && canCreate ? 'Escolha outro modo e confirme. Treinamento e Personalizada saem deste lobby e abrem um novo.' : restriction}
        descriptionId={descriptionId} feedback={feedbackLine} tutorial={state.tutorial} summonerName={summoner?.gameName || summoner?.displayName}
        lobby={lobby ? { queueId: lobby.queueId, queueName: lobbyQueueName(lobby, state.queues), onBack: () => setChangingMode(false) } : undefined}
        onCreate={run} onStart={run}
        onCreateCustom={options => run({ type: 'createCustomGame', options })}
        customJoin={{ games: state.customGames, send,
          onJoin: (partyId, password) => run({ type: 'joinCustomGame', partyId, ...(password ? { password } : {}) }) }} />
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
  // Posições que faltam na fila agora (#410): marcadas na escolha, como no client oficial.
  const scarce = lobby.scarcePositions;
  const freeSlots = Math.max(0, lobby.maxLobbySize - lobby.members.length);
  // Rotas (#613): os chips dos cards só mostram. Embaixo de você, um card pequeno com as duas rotas;
  // passar o mouse (ou o foco do teclado) numa delas abre a lista para trocar.
  const positionFieldset = (
          <fieldset aria-label="Suas posições" disabled={!canSetPositions}
            className="relative z-20 mx-auto flex w-fit items-center gap-1 rounded-full border border-accent/40 bg-surface p-1 shadow-soft animate-fade-up @min-[60rem]:translate-x-7">
            <legend className="sr-only">Suas posições</legend>
            {!localMember && <p className="sr-only">Aguarde a identificação do seu membro no lobby.</p>}
            {phase !== 'Lobby' && <p className="sr-only">A escolha de posições está disponível na fase de lobby.</p>}
            <LaneSlot label="Posição primária" value={positions.first} scarce={scarce} disabled={!canSetPositions}
              missing={canSetPositions && positions.first === 'unselected'}
              allowed={() => true} clearable
              onChange={first => {
                choosePositions({ first, second: validPositions(first, positions.second) ? positions.second : 'unselected' });
              }} />
            <LaneSlot label="Posição secundária" value={positions.second} scarce={scarce}
              disabled={!canSetPositions || positions.first === 'unselected' || positions.first === 'fill'}
              missing={canSetPositions && positions.first !== 'unselected' && !positionsComplete(positions.first, positions.second)}
              allowed={position => validPositions(positions.first, position)}
              onChange={second => choosePositions({ first: positions.first, second })} />
            {scarce.length > 0 && (
              <p className="sr-only">Prioritárias: {scarce.map(position => positionNames[position]).join(', ')}. Faltam na fila agora.</p>
            )}
            {!positionsComplete(positions.first, positions.second) && (
              <p className="sr-only">Escolha a posição secundária para salvar.</p>
            )}
          </fieldset>
  );
  // Palco (#600): você no centro; os outros e as vagas livres se espalham a partir do meio.
  const others = lobby.members.filter(member => member.puuid !== summoner?.puuid);
  const seats = stageSeats(localMember, others, freeSlots);
  return (
    <section aria-label="Lobby" onDragOver={onDragOver} onDragLeave={onDragLeave} onDrop={onDrop}
      className={cx('flex w-full shrink-0 flex-col gap-3 rounded-panel text-ink transition-shadow',
        dropping && 'ring-2 ring-accent')}>
      {/* Faixa do modo (#600): a fila em destaque e as ações do lobby logo abaixo. */}
      {/* Faixa do modo (#600), numa linha só (#607): a fila à esquerda e as ações à direita, para sobrar altura para o palco. */}
      <header className="lobby-banner relative flex flex-wrap items-center justify-between gap-x-4 gap-y-2 overflow-hidden rounded-panel surface card px-5 py-2.5 shadow-soft animate-fade-up">
        <div className="min-w-0">
          <h2 className="sr-only">Lobby</h2>
          <p className="text-title font-semibold leading-tight text-balance"><span className="sr-only">Fila atual: </span><strong className="font-semibold">{queueLabel}</strong></p>
          <p className="mt-0.5 flex flex-wrap items-center gap-x-2 text-caption text-muted">
            <span className="font-semibold text-accent-ink">{lobby.members.length} / {lobby.maxLobbySize} jogadores</span>
            <span aria-hidden="true">·</span>
            <span>{lobby.isLeader ? 'Você é o líder' : 'Você é membro'}</span>
          </p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
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
      // Você fica no meio, embaixo do botão de buscar partida (#613): a faixa de amigos à direita tira 3,5rem
      // do conteúdo, então o palco anda metade disso; uma vaga vazia equilibra os lados quando o número é par.
      <ul aria-label="Jogadores no lobby" className="flex flex-wrap items-stretch justify-center gap-x-2.5 gap-y-4 px-1 @min-[60rem]:translate-x-7 @min-[60rem]:gap-x-3">
        {[...seats, ...(localMember && seats.length % 2 === 0 ? [null] : [])].map((seat, order) => {
          if (seat === null) {
            return <li key="equilibrio" role="presentation" aria-hidden="true"
              className={cx('invisible mt-14 @min-[80rem]:mt-[4.5rem]', lobby.showQuickPlaySlots ? 'w-60' : SEAT_WIDTH)} />;
          }
          if (seat.kind === 'free') {
            const index = seat.index;
            return (
              <li key={`vaga-${index}`} aria-label="Vaga livre" style={{ animationDelay: `${order * 70}ms` }}
                className={cx('relative mt-14 flex flex-col items-center justify-center gap-2 rounded-panel border-2 border-dashed border-line px-2 py-6 text-muted animate-fade-up @min-[80rem]:mt-[4.5rem]', SEAT_WIDTH)}>
                {/* O "+" abre a lista de amigos para convidar (#206); sem permissão de convidar, fica só o desenho. */}
                {lobby.canInvite ? (
                  <button type="button" aria-label="Convidar amigo" title="Convidar amigo" aria-expanded={picker === index}
                    aria-haspopup="dialog" onClick={() => setPicker(picker === index ? null : index)}
                    className={`flex size-16 items-center justify-center rounded-full bg-raised text-ink transition-colors hover:bg-accent hover:text-on-accent ${focusRing}`}>
                    <PlusIcon size={22} />
                  </button>
                ) : <span aria-hidden="true" className="flex size-16 items-center justify-center rounded-full bg-raised"><PlusIcon size={22} /></span>}
                <span className="text-body font-medium">{lobby.canInvite ? 'Convidar' : 'Vaga livre'}</span>
              </li>
            );
          }
          const member = seat.member;
          const me = member.puuid === summoner?.puuid;
          const name = me ? localName : memberName(member, state.friends);
          const isAway = away.has(member.puuid);
          return (
            <li key={member.puuid} style={{ animationDelay: `${order * 70}ms` }}
              className={cx('group relative mt-14 flex flex-col items-center gap-2.5 rounded-panel border bg-surface px-2.5 pb-4 text-center @min-[80rem]:mt-[4.5rem] @min-[80rem]:gap-3 @min-[80rem]:px-4 @min-[80rem]:pb-5 shadow-soft transition-all duration-200 animate-fade-up',
                // Card por jogador (#607): o seu com a borda de destaque, por cima dos outros para a escolha de campeão aberta nele não ficar escondida.
                me ? 'z-10 border-accent ring-1 ring-accent/40' : 'border-line hover:-translate-y-0.5 hover:shadow-lift',
                lobby.showQuickPlaySlots ? 'w-60' : SEAT_WIDTH)}>
              {/* O jogador inteiro abre o perfil dele (#308). O botão fica por baixo dos
                  controles (menu do líder, campeões do Jogo Dinâmico), que vêm depois e são posicionados. */}
              {!me && onOpenProfile && (
                <button type="button" aria-label={`Ver perfil de ${name}`} title={`Ver perfil de ${name}`}
                  onClick={() => onOpenProfile(member.puuid)}
                  className={`absolute inset-0 cursor-pointer rounded-panel transition-colors hover:bg-raised/60 ${focusRing}`} />
              )}
              {/* O ícone sai pela borda de cima do card, com o mesmo tamanho em todos: as linhas ficam alinhadas (#607). */}
              <div className={cx('relative -mt-14 shrink-0 rounded-full @min-[80rem]:-mt-[4.5rem] transition-opacity', me ? 'bg-primary-gradient p-1' : 'bg-line p-1', isAway && 'opacity-40 grayscale')}>
                <Avatar name={me && summoner?.gameName ? summoner.gameName : name} src={assetUrl('profileIcon', member.summonerIconId)}
                  size={96} className="ring-4 ring-surface @min-[80rem]:size-32! @min-[80rem]:text-[2.875rem]!" />
                {/* Nível preso embaixo do ícone, como no client oficial: uma linha a menos no palco (#603). */}
                <span className="absolute -bottom-2 left-1/2 -translate-x-1/2 whitespace-nowrap rounded-full bg-raised px-2 py-0.5 text-caption font-semibold text-muted ring-2 ring-surface">Nível {member.summonerLevel}</span>
              </div>
              <div className={cx('mt-2 flex w-full min-w-0 items-center justify-center gap-1 transition-opacity', isAway && 'opacity-50')}>
                {/* Nick e tag numa linha só, cortando com "…" quando não cabem (#613); o nome inteiro fica no título. */}
                <h3 title={name} className="min-w-0 truncate text-section font-semibold leading-tight @min-[80rem]:text-title">
                  {splitName(name).nick}
                  {splitName(name).tag && <span className="font-medium text-muted">{splitName(name).tag}</span>}
                </h3>
                {member.isLeader && (
                  <span role="img" aria-label="Líder do grupo" title="Líder do grupo"
                    className="flex size-6 shrink-0 items-center justify-center rounded-full bg-emerald-soft text-warn">
                    <CrownIcon size={14} />
                  </span>
                )}
              </div>
              {me && <p className="sr-only">Você</p>}
              {isAway && (
                <p className="inline-flex items-center gap-1.5 rounded-full bg-emerald-soft px-2.5 py-1 text-caption font-semibold text-warn">
                  <span aria-hidden="true" className="tracking-widest">•••</span>
                  Ainda não voltou ao lobby
                </p>
              )}
              {blocking.has(member.puuid) && (
                <p title={blocking.get(member.puuid)!.join(' ')}
                  className="inline-flex max-w-full items-center gap-1.5 rounded-full bg-emerald-soft px-2.5 py-1 text-caption font-semibold text-warn">
                  <span aria-hidden="true">!</span>
                  <span className="truncate">Bloqueando a fila<span className="sr-only">: {blocking.get(member.puuid)!.join(' ')}</span></span>
                </p>
              )}
              <div className={cx('flex flex-wrap items-center justify-center gap-1.5 transition-opacity', isAway && 'opacity-50')}>
                <LobbyRank ranked={member.ranked} queueId={lobby.queueId} />
              </div>
              {lobby.showQuickPlaySlots && (
                <div className="relative mt-1">
                  <QuickPlaySlots key={`${lobby.queueId}:${member.puuid}`} member={member} mine={me} canEdit={canSetSlots}
                    state={state} send={send} notes={me ? slotNotes : undefined} />
                </div>
              )}
              {/* Rift Clássico (#230): a 1ª preferência em destaque e as outras em ordem. */}
              {lobby.showPositionSelector && lobby.orderedPositions && <MemberPositionOrder member={member} name={name} />}
              {lobby.showPositionSelector && !lobby.orderedPositions && (
                // As rotas ficam no pé do card, na mesma altura em todos (#607).
                <div className="mt-auto pt-1">
                  <ul aria-label={`Posições de ${name}`} className="flex flex-wrap items-center justify-center gap-1.5">
                  {[member.firstPosition, member.secondPosition].map((position, slot) => (
                    <li key={slot} title={`${slot === 0 ? 'Primária' : 'Secundária'}: ${positionNames[position]}`}
                      className={cx('flex items-center gap-1 rounded-full px-2.5 py-1 text-caption font-semibold @min-[80rem]:px-3 @min-[80rem]:py-1.5 @min-[80rem]:text-body',
                        slot === 0 ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-muted')}>
                      <PositionIcon position={position} size={16} />
                      <span className={slot === 0 ? 'truncate' : 'sr-only @min-[60rem]:not-sr-only @min-[60rem]:truncate'}>{positionNames[position]}</span>{' '}
                      <span className="sr-only">{slot === 0 ? 'Primária' : 'Secundária'}</span>
                    </li>
                  ))}
                </ul>
                </div>
              )}
              {/* Tornar líder e remover do grupo (#267): só o líder, nos outros jogadores. */}
              {lobby.isLeader && <div className="absolute right-0 top-0 z-10">
                {me
                  // No próprio card não há menu: o espaço do "⋯" fica reservado, como nos outros.
                  ? <span aria-hidden="true" data-testid="lobby-menu-spacer" className="block size-8" />
                  : <LobbyMemberMenu name={name} summonerId={member.summonerId} send={send} disabled={!canManageMembers} allowedInvite={member.allowedInviteOthers} />}
              </div>}
            </li>
          );
        })}
      </ul>
      )}
      {invites.error && picker === null && <p role="alert" className="text-body text-danger">{invites.error}</p>}
      {picker !== null && <InvitePicker invites={invites} groups={state.friendGroups} recent={state.recentPlayers}
        loadRecent={loadRecent} suggested={state.suggestedPlayers} loadSuggested={loadSuggested}
        dismissSuggested={dismissSuggested} onClose={closePicker} />}

      {/* Card de escolha das rotas, sempre embaixo de você (#607). */}
      {lobby.showPositionSelector && !lobby.orderedPositions && !lobby.subteams && positionFieldset}
      {/* Fila sem posições (ARAM, Arena, Co-op, eventos): nada no lugar, como no client oficial. A fila troca pelo "Trocar modo" (#231). */}
      {lobby.showPositionSelector && lobby.orderedPositions ? (
        <PositionOrderCard member={localMember} scarce={scarce} canEdit={canSetPositions} send={send} />
      ) : !lobby.showPositionSelector && lobby.showQuickPlaySlots ? (
        <div className="flex min-w-0 flex-col gap-2 rounded-panel surface card p-4 shadow-soft animate-fade-up">
          <h3 className="px-1 text-section font-semibold">Suas posições e campeões</h3>
          <p className="px-1 text-caption text-muted">No Jogo Dinâmico, escolha as duas posições e um campeão para cada uma no seu card, acima.</p>
          <div ref={setSlotNotes} className="flex flex-col gap-1 px-1" />
        </div>
      ) : null}
      {/* O aviso padrão de Trocar modo fica só para o leitor de tela: a tela do lobby tem altura fixa (#603). */}
      <p id={descriptionId} className={restriction === CHANGE_MODE_HINT ? 'sr-only' : 'text-center text-caption text-muted'}>{restriction}</p>
      {feedbackLine}
    </section>
  );
}

// Largura de cada vaga do palco (#613): fixa na janela estreita; com espaço, as cinco dividem a área do lobby
// (descontando o deslocamento para o meio da janela e os espaços entre elas), até 19rem cada.
const SEAT_WIDTH = 'w-40 @min-[60rem]:w-[min(19rem,calc((100%_-_5.5rem)/5))]';

const PICKER_POSITIONS = ['top', 'jungle', 'middle', 'bottom', 'utility', 'fill'] as const satisfies readonly Position[];
const shortNames: Record<(typeof PICKER_POSITIONS)[number], string> = {
  top: 'Topo', jungle: 'Selva', middle: 'Meio', bottom: 'Atirador', utility: 'Suporte', fill: 'Preencher',
};

type PickerProps = {
  label: string;
  value: Position;
  scarce: Position[];
  disabled: boolean;
  allowed: (position: Position) => boolean;
  /** Clicar na rota escolhida de novo volta para "Não selecionado". */
  clearable?: boolean;
  onChange: (position: Position) => void;
};

/** Escolha de uma rota (#600): as cinco rotas e Preencher, lado a lado, com a escolhida em destaque. */
function PositionPicker({ label, value, scarce, disabled, allowed, clearable = false, onChange }: PickerProps) {
  return (
    <fieldset role="radiogroup" aria-label={label} disabled={disabled} data-value={value} className="flex min-w-0">
      <div className="grid min-w-0 flex-1 grid-cols-6 gap-1 rounded-control bg-raised p-1">
        {PICKER_POSITIONS.map(position => {
          const checked = value === position;
          const blocked = !allowed(position);
          return (
            <button key={position} type="button" role="radio" aria-checked={checked} data-value={position}
              aria-label={`${positionNames[position]}${scarce.includes(position) ? ' (prioritária)' : ''}`}
              title={`${positionNames[position]}${scarce.includes(position) ? ' (prioritária: falta na fila agora)' : ''}`} disabled={blocked} aria-disabled={blocked || undefined}
              onClick={() => onChange(checked && clearable ? 'unselected' : position)}
              className={cx('relative flex min-w-0 flex-col items-center gap-0.5 rounded-control-sm px-1 py-1.5 text-[0.6875rem] font-semibold transition-colors',
                'focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent disabled:cursor-not-allowed',
                checked ? 'bg-surface text-accent-ink shadow-soft' : 'text-muted enabled:hover:text-ink',
                blocked && 'opacity-35')}>
              <PositionIcon position={position} size={18} />
              <span className="max-w-full truncate">{shortNames[position]}</span>
              {scarce.includes(position) && <ScarceMark className="absolute right-1 top-1" />}
            </button>
          );
        })}
      </div>
    </fieldset>
  );
}

/**
 * Uma rota do card pequeno (#613): mostra a escolhida; passar o mouse ou focar abre a lista, para cima, sobre o seu card.
 * Sair com o mouse fecha mesmo com o foco dentro (depois de um clique), mas não tira o foco de quem usa o teclado.
 */
function LaneSlot({ missing = false, ...picker }: PickerProps & { missing?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const { label, value, scarce, disabled } = picker;
  const name = value === 'unselected' ? 'Escolher' : positionNames[value];
  return (
    <div ref={ref} className="group/lane relative"
      onMouseLeave={() => {
        const active = document.activeElement;
        if (active instanceof HTMLElement && ref.current?.contains(active)) active.blur();
      }}>
      <button type="button" disabled={disabled} aria-label={`${label}: ${positionNames[value]}`} title={`${label.replace('Posição ', '').replace(/^./, c => c.toUpperCase())}: ${positionNames[value]}`}
        className={cx('flex items-center gap-1.5 rounded-full px-3.5 py-1.5 text-body font-semibold transition-colors disabled:cursor-not-allowed @min-[80rem]:px-5 @min-[80rem]:py-2 @min-[80rem]:text-section',
          'focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent',
          label === 'Posição primária' ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-ink',
          missing && 'ring-2 ring-accent animate-pulse',
          disabled ? 'opacity-50' : 'group-hover/lane:bg-accent group-hover/lane:text-on-accent')}>
        <PositionIcon position={value} size={18} />
        <span className="whitespace-nowrap">{name}</span>
        {value !== 'unselected' && scarce.includes(value) && <ScarceMark />}
      </button>
      <div className={cx('invisible absolute bottom-full left-1/2 z-30 -translate-x-1/2 pb-2 opacity-0 transition-opacity',
        !disabled && 'group-hover/lane:visible group-hover/lane:opacity-100 group-focus-within/lane:visible group-focus-within/lane:opacity-100')}>
        <div className="w-[27rem] rounded-panel border border-line bg-surface p-1.5 shadow-lift">
          <PositionPicker {...picker} />
        </div>
      </div>
    </div>
  );
}

/** "Nick#TAG" em nick e "#TAG", para a tag ir mais apagada no card (#607). */
function splitName(name: string): { nick: string; tag: string } {
  const at = name.lastIndexOf('#');
  return at > 0 ? { nick: name.slice(0, at), tag: name.slice(at) } : { nick: name, tag: '' };
}

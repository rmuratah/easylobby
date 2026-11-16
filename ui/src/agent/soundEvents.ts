import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectSwap } from '../bindings/ChampSelectSwap';
import type { GameflowPhase } from '../bindings/GameflowPhase';
import type { SoundCue } from './sound';

/** O que a tela sabe além do estado: janela com foco e conversa aberta à vista. */
export type SoundContext = {
  focused: boolean;
  openConversationId: string | null;
};

/** Fases em que a seleção acabou porque a partida começou, não por um dodge. */
const AFTER_CHAMP_SELECT: ReadonlySet<GameflowPhase | null> = new Set<GameflowPhase | null>([
  'ChampSelect', 'GameStart', 'FailedToLaunch', 'InProgress', 'Reconnect', 'WaitingForStats', 'PreEndOfGame', 'EndOfGame',
]);

/**
 * Sons que a mudança de `prev` para `next` pede (#501), na ordem em que
 * tocam. Sem estado anterior, ou com outra sessão do LoL ou outro invocador,
 * nada toca: o que já existia ao conectar não é novidade.
 *
 * Partida encontrada, contagem da vez e a confirmação do próprio ban ou
 * escolha tocam em outros lugares (#135, #475, #498).
 */
export function soundCues(prev: AgentState | null, next: AgentState | null, context: SoundContext): SoundCue[] {
  if (!prev || !next || !sameSession(prev, next)) return [];
  const cues: SoundCue[] = [];
  const add = (cue: SoundCue, when: boolean) => { if (when && !cues.includes(cue)) cues.push(cue); };

  const before = prev.gameflowPhase;
  const now = next.gameflowPhase;

  // 5. Partida cancelada: o aceite voltou para a fila (alguém recusou) ou a seleção acabou sem a partida (dodge).
  // Quem recusou já ouviu o próprio clique.
  const readyCheckCancelled = before === 'ReadyCheck' && now !== 'ReadyCheck' && now !== 'ChampSelect'
    && prev.readyCheck?.playerResponse !== 'Declined';
  const dodged = before === 'ChampSelect' && !AFTER_CHAMP_SELECT.has(now);
  add('cancelled', readyCheckCancelled || dodged);

  // 3. Entrou na seleção de campeões.
  const enteredChampSelect = before !== 'ChampSelect' && now === 'ChampSelect';
  add('champSelectStart', enteredChampSelect);

  const was = prev.champSelect;
  const is = next.champSelect;
  if (is && now === 'ChampSelect') {
    const sameChampSelect = was !== null && was.id === is.id;
    // 2. Começou a sua vez. Ao entrar na seleção, o som de entrada já avisa.
    add('turnStart', !enteredChampSelect && is.pendingAction !== null
      && (!sameChampSelect || was.pendingAction?.id !== is.pendingAction.id));
    if (sameChampSelect) {
      // 7. Pedido de troca recebido.
      add('swapRequest', newlyReceived(was.trades, is.trades) || newlyReceived(was.pickOrderSwaps, is.pickOrderSwaps)
        || newlyReceived(was.positionSwaps, is.positionSwaps));
      // 8. Aliado confirmou o campeão.
      add('allyLockIn', allyLockedIn(was, is));
    }
  }

  // 9. Fim da seleção / partida carregando.
  const wasStarting = was?.timer.phase === 'GAME_STARTING';
  const startingNow = is?.timer.phase === 'GAME_STARTING' && now === 'ChampSelect';
  add('gameStart', !wasStarting && (startingNow || (before === 'ChampSelect' && now === 'GameStart')));

  // 4. Convite de lobby recebido.
  const knownInvites = new Set(prev.receivedInvitations.map(i => i.invitationId));
  add('invite', next.receivedInvitations.some(i => !knownInvites.has(i.invitationId)));

  // 6. Mensagem recebida: conversa privada que não está à vista, e o chat da seleção.
  add('message', newPrivateMessage(prev, next, context) || newChampSelectMessage(prev, next));

  // 10. Alguém entrou ou saiu do lobby.
  if (prev.lobby && next.lobby && now === 'Lobby') {
    const me = next.summoner?.puuid;
    const others = (lobby: NonNullable<AgentState['lobby']>) => new Set(lobby.members.map(m => m.puuid).filter(p => p !== me));
    const a = others(prev.lobby);
    const b = others(next.lobby);
    add('memberJoin', [...b].some(p => !a.has(p)));
    add('memberLeave', [...a].some(p => !b.has(p)));
  }
  return cues;
}

function newlyReceived(before: ChampSelectSwap[], after: ChampSelectSwap[]): boolean {
  const received = new Set(before.filter(s => s.state === 'RECEIVED').map(s => s.id));
  return after.some(s => s.state === 'RECEIVED' && !received.has(s.id));
}

function allyLockedIn(before: ChampSelect, after: ChampSelect): boolean {
  const done = new Set(before.actions.filter(a => a.completed).map(a => a.id));
  return after.actions.some(a => a.type === 'pick' && a.completed && a.isAllyAction
    && a.actorCellId !== after.localPlayerCellId && !done.has(a.id));
}

function newPrivateMessage(prev: AgentState, next: AgentState, { focused, openConversationId }: SoundContext): boolean {
  if (prev.conversations === next.conversations) return false;
  return next.conversations.some(conversation => {
    if (focused && conversation.id === openConversationId) return false;
    const known = new Set(prev.conversations.find(c => c.id === conversation.id)?.messages.map(m => m.id));
    return conversation.messages.some(m => !m.fromMe && !known.has(m.id));
  });
}

function newChampSelectMessage(prev: AgentState, next: AgentState): boolean {
  if (!prev.champSelectChat || !next.champSelectChat || prev.champSelectChat === next.champSelectChat) return false;
  const known = new Set(prev.champSelectChat.messages.map(m => m.id));
  return next.champSelectChat.messages.some(m => !m.fromMe && !known.has(m.id));
}

function sameSession(a: AgentState, b: AgentState): boolean {
  const session = (s: AgentState) => s.lcu.state === 'connected' ? s.lcu.session : null;
  return session(a) !== null && session(a) === session(b) && a.summoner?.puuid === b.summoner?.puuid;
}

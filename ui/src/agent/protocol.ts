import type { ServerMessage } from '../bindings/ServerMessage';
import type { AgentState } from '../bindings/AgentState';
import type { LcuStatus } from '../bindings/LcuStatus';
import type { Summoner } from '../bindings/Summoner';
import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { Position } from '../bindings/Position';
import { isQueue } from './queues';
import type { ErrorCode } from '../bindings/ErrorCode';
import type { ReadyCheck } from '../bindings/ReadyCheck';
import type { Matchmaking } from '../bindings/Matchmaking';
import type { MatchmakingError } from '../bindings/MatchmakingError';
import type { SearchState } from '../bindings/SearchState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import type { Conversation } from '../bindings/Conversation';
import type { ChampSelectChat } from '../bindings/ChampSelectChat';
import type { RunePages } from '../bindings/RunePages';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { Friend } from '../bindings/Friend';

// Exige atualizar a validação quando o contrato gerado ganhar ou remover códigos.
const errorCodes = {
  notConnected: true,
  lcu: true,
  launchFailed: true,
  invalidCommand: true,
} satisfies Record<ErrorCode, true>;

function errorCode(v: unknown): v is ErrorCode {
  return typeof v === 'string' && Object.hasOwn(errorCodes, v);
}

const object = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;
const uint = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= 0;
const nullableString = (v: unknown) => v === null || typeof v === 'string';
function summoner(v: unknown): v is Summoner | null {
  return v === null || (object(v) && ['gameName', 'tagLine', 'displayName', 'puuid'].every(k => typeof v[k] === 'string') &&
    ['summonerLevel', 'profileIconId', 'summonerId'].every(k => uint(v[k])));
}
const positions = {
  top: true, jungle: true, middle: true, bottom: true, utility: true, fill: true, unselected: true,
} satisfies Record<Position, true>;
const position = (v: unknown): v is Position => typeof v === 'string' && Object.hasOwn(positions, v);
function lobbyMember(v: unknown): v is LobbyMember {
  return object(v) && typeof v.puuid === 'string' &&
    ['summonerId', 'summonerIconId', 'summonerLevel'].every(k => uint(v[k])) &&
    typeof v.isLeader === 'boolean' && position(v.firstPosition) && position(v.secondPosition);
}
function lobby(v: unknown): v is Lobby | null {
  return v === null || (object(v) && uint(v.queueId) && (v.queue === null || isQueue(v.queue)) &&
    uint(v.maxLobbySize) && typeof v.isLeader === 'boolean' && typeof v.canStartActivity === 'boolean' &&
    typeof v.showPositionSelector === 'boolean' && Array.isArray(v.members) && v.members.every(lobbyMember));
}
const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const nums = (v: Record<string, unknown>, keys: string[]) => keys.every(k => num(v[k]));
const strings = (v: Record<string, unknown>, keys: string[]) => keys.every(k => typeof v[k] === 'string');
const uints = (v: Record<string, unknown>, keys: string[]) => keys.every(k => uint(v[k]));
const booleans = (v: Record<string, unknown>, keys: string[]) => keys.every(k => typeof v[k] === 'boolean');
const arrays = (v: Record<string, unknown>, keys: string[]) => keys.every(k => Array.isArray(v[k]));
const list = <T>(v: unknown, item: (x: unknown) => x is T): v is T[] => Array.isArray(v) && v.every(item);

// Os eventos abaixo são conferidos no primeiro nível: tipos dos campos
// simples, listas e ids usados para juntar o evento ao estado. O conteúdo
// das listas internas (ações do draft, runas...) fica para as telas.
function readyCheck(v: unknown): v is ReadyCheck | null {
  return v === null || (object(v) && strings(v, ['state', 'playerResponse']) && nums(v, ['timer', 'duration']));
}
const searchStates = {
  Searching: true, Found: true, Error: true, AbandonedLowPriorityQueue: true, Unknown: true,
} satisfies Record<SearchState, true>;
const searchState = (v: unknown): v is SearchState => typeof v === 'string' && Object.hasOwn(searchStates, v);
function matchmakingError(v: unknown): v is MatchmakingError {
  return object(v) && strings(v, ['errorType', 'message']) && num(v.penaltyTimeRemaining);
}
function matchmaking(v: unknown): v is Matchmaking | null {
  return v === null || (object(v) && searchState(v.searchState) &&
    uint(v.queueId) &&
    nums(v, ['timeInQueue', 'estimatedQueueTime', 'lowPriorityTimeRemaining']) && list(v.errors, matchmakingError));
}
function champSelect(v: unknown): v is ChampSelect | null {
  return v === null || (object(v) && typeof v.id === 'string' && uint(v.queueId) && Number.isSafeInteger(v.localPlayerCellId) &&
    (v.queue === null || isQueue(v.queue)) && booleans(v, ['isCustomGame', 'hasSimultaneousBans', 'hasSimultaneousPicks']) &&
    object(v.timer) && typeof v.timer.phase === 'string' && object(v.bans) && arrays(v.bans, ['myTeam', 'theirTeam']) &&
    arrays(v, ['myTeam', 'theirTeam', 'actions']) && (v.pendingAction === null || object(v.pendingAction)));
}
function receivedInvitation(v: unknown): v is ReceivedInvitation {
  return object(v) && strings(v, ['invitationId', 'fromSummonerName']) && uints(v, ['fromSummonerId', 'queueId']) &&
    (v.queue === null || isQueue(v.queue)) && typeof v.canAccept === 'boolean';
}
function conversation(v: unknown): v is Conversation {
  return object(v) && strings(v, ['id', 'gameName', 'tagLine']) && uint(v.unreadCount) && list(v.messages, object);
}
function champSelectChat(v: unknown): v is ChampSelectChat | null {
  return v === null || (object(v) && list(v.messages, object));
}
function runePages(v: unknown): v is RunePages | null {
  return v === null || (object(v) && list(v.pages, object) && (v.currentPageId === null || uint(v.currentPageId)) &&
    typeof v.canCreatePage === 'boolean');
}
function runeCatalog(v: unknown): v is RuneCatalog | null {
  return v === null || (object(v) && list(v.styles, object) && list(v.perks, object));
}
function friend(v: unknown): v is Friend {
  return object(v) && strings(v, ['id', 'puuid', 'gameName', 'tagLine', 'name', 'availability', 'statusMessage']) &&
    (v.profileIconId === null || uint(v.profileIconId)) && (v.activity === null || object(v.activity));
}
function status(v: unknown): v is LcuStatus {
  if (!object(v)) return false;
  switch (v.state) {
    case 'starting': return true;
    case 'connected': return uint(v.session);
    case 'disconnected': return typeof v.reason === 'string';
    case 'retrying': return uint(v.attempt) && uint(v.retryInMs) && typeof v.clientNotRunning === 'boolean' && typeof v.error === 'string';
    default: return false;
  }
}
function state(v: unknown): v is AgentState {
  return object(v) && status(v.lcu) && summoner(v.summoner) && nullableString(v.gameflowPhase) && lobby(v.lobby) &&
    readyCheck(v.readyCheck) && matchmaking(v.matchmaking) && champSelect(v.champSelect) &&
    list(v.receivedInvitations, receivedInvitation) && list(v.conversations, conversation) &&
    runePages(v.runePages) && runeCatalog(v.runeCatalog) && list(v.friends, friend);
}

// Os tipos vêm do Rust; esta validação protege a fronteira JSON em runtime.
// Mensagens novas são ignoradas para permitir extensões compatíveis do protocolo.
export function parseMessage(text: string): ServerMessage | null {
  const v: unknown = JSON.parse(text);
  if (!object(v) || typeof v.type !== 'string') throw new Error('Mensagem inválida do agente.');
  let valid: boolean;
  switch (v.type) {
    case 'hello': valid = uint(v.protocol) && typeof v.agentVersion === 'string' && state(v.state); break;
    case 'lcu': valid = status(v.status); break;
    case 'summoner': valid = summoner(v.summoner); break;
    case 'lobby': valid = lobby(v.lobby); break;
    case 'gameflowPhase': valid = nullableString(v.phase); break;
    case 'readyCheck': valid = readyCheck(v.readyCheck); break;
    case 'matchmaking': valid = matchmaking(v.matchmaking); break;
    case 'champSelect': valid = champSelect(v.champSelect); break;
    case 'receivedInvitations': valid = list(v.invitations, receivedInvitation); break;
    case 'conversation': valid = conversation(v.conversation); break;
    case 'conversationClosed': valid = typeof v.id === 'string'; break;
    case 'champSelectChat': valid = champSelectChat(v.champSelectChat); break;
    case 'runePages': valid = runePages(v.runePages); break;
    case 'runeCatalog': valid = runeCatalog(v.runeCatalog); break;
    case 'friends': valid = list(v.friends, friend); break;
    case 'response': valid = uint(v.id) && object(v.result) && (v.result.status === 'ok' ||
      (v.result.status === 'error' && errorCode(v.result.code) && typeof v.result.message === 'string')); break;
    case 'invalidMessage': valid = typeof v.message === 'string'; break;
    default: return null;
  }
  if (!valid) throw new Error('Mensagem inválida do agente.');
  return v as ServerMessage;
}

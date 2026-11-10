import type { ReadyCheckState } from '../bindings/ReadyCheckState';
import type { ReadyCheckResponse } from '../bindings/ReadyCheckResponse';
import type { ServerMessage } from '../bindings/ServerMessage';
import type { PatchNotes } from '../bindings/PatchNotes';
import type { LiveGame } from '../bindings/LiveGame';
import type { AgentState } from '../bindings/AgentState';
import type { LcuStatus } from '../bindings/LcuStatus';
import type { Summoner } from '../bindings/Summoner';
import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { PlayerSlot } from '../bindings/PlayerSlot';
import type { LobbyRestriction } from '../bindings/LobbyRestriction';
import type { BotDifficulty } from '../bindings/BotDifficulty';
import type { CustomBot } from '../bindings/CustomBot';
import type { CustomLobby } from '../bindings/CustomLobby';
import type { CustomMember } from '../bindings/CustomMember';
import type { TutorialPart } from '../bindings/TutorialPart';
import type { LobbyInvitation } from '../bindings/LobbyInvitation';
import type { InvitationState } from '../bindings/InvitationState';
import type { Position } from '../bindings/Position';
import { isQueue } from './queues';
import type { ErrorCode } from '../bindings/ErrorCode';
import type { ReadyCheck } from '../bindings/ReadyCheck';
import type { Matchmaking } from '../bindings/Matchmaking';
import type { MatchmakingError } from '../bindings/MatchmakingError';
import type { SearchState } from '../bindings/SearchState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectChat } from '../bindings/ChampSelectChat';
import type { ChampSelectChatMessage } from '../bindings/ChampSelectChatMessage';
import type { LobbyChat } from '../bindings/LobbyChat';
import type { LobbyLink } from '../bindings/LobbyLink';
import type { LobbyChatMessage } from '../bindings/LobbyChatMessage';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import type { Conversation } from '../bindings/Conversation';
import type { RunePages } from '../bindings/RunePages';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { Friend } from '../bindings/Friend';
import type { FriendGroup } from '../bindings/FriendGroup';
import type { SummonerSpellCatalog } from '../bindings/SummonerSpellCatalog';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';
import type { ChampSelectChampions } from '../bindings/ChampSelectChampions';
import type { GameQueue } from '../bindings/GameQueue';
import type { RankedStats } from '../bindings/RankedStats';
import type { RankedEntry } from '../bindings/RankedEntry';
import type { ChampionMastery } from '../bindings/ChampionMastery';
import type { MatchHistory } from '../bindings/MatchHistory';
import type { MatchSummary } from '../bindings/MatchSummary';
import type { MatchDetails } from '../bindings/MatchDetails';
import type { PlayerProfile } from '../bindings/PlayerProfile';
import type { MatchPlayer } from '../bindings/MatchPlayer';
import type { MatchResult } from '../bindings/MatchResult';
import type { RecentPlayer } from '../bindings/RecentPlayer';
import type { CustomGameListing } from '../bindings/CustomGameListing';

// Exige atualizar a validação quando o contrato gerado ganhar ou remover códigos.
const errorCodes = {
  notConnected: true,
  lcu: true,
  launchFailed: true,
  invalidCommand: true,
  notFound: true,
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
// Jogo Dinâmico (#218).
function playerSlot(v: unknown): v is PlayerSlot {
  return object(v) && position(v.position) &&
    ['championId', 'skinId', 'spell1Id', 'spell2Id', 'runeStyleId', 'runeSubStyleId', 'keystoneId'].every(k => uint(v[k]));
}
function lobbyMember(v: unknown): v is LobbyMember {
  return object(v) && typeof v.puuid === 'string' &&
    ['summonerId', 'summonerIconId', 'summonerLevel'].every(k => uint(v[k])) &&
    typeof v.isLeader === 'boolean' && position(v.firstPosition) && position(v.secondPosition) &&
    Array.isArray(v.playerSlots) && v.playerSlots.every(playerSlot);
}
const invitationStates = {
  Requested: true, Pending: true, Accepted: true, Declined: true, Kicked: true, OnHold: true, Error: true, Unknown: true,
} satisfies Record<InvitationState, true>;
function lobbyInvitation(v: unknown): v is LobbyInvitation {
  return object(v) && typeof v.puuid === 'string' && uint(v.summonerId) &&
    typeof v.state === 'string' && Object.hasOwn(invitationStates, v.state);
}
function lobbyRestriction(v: unknown): v is LobbyRestriction {
  return object(v) && typeof v.code === 'string' && (v.message === null || typeof v.message === 'string') &&
    Array.isArray(v.puuids) && v.puuids.every(p => typeof p === 'string');
}
const botDifficulties = { intro: true, beginner: true, intermediate: true, unknown: true } satisfies Record<BotDifficulty, true>;
function customBot(v: unknown): v is CustomBot {
  return object(v) && typeof v.uuid === 'string' && uint(v.championId) && position(v.position) &&
    typeof v.difficulty === 'string' && Object.hasOwn(botDifficulties, v.difficulty);
}
function customMember(v: unknown): v is CustomMember {
  return object(v) && typeof v.puuid === 'string' && uint(v.summonerId) && uint(v.summonerIconId) &&
    typeof v.isLeader === 'boolean' && (v.bot === null || customBot(v.bot));
}
function customGameSettings(v: unknown): boolean {
  return object(v) && typeof v.name === 'string' && typeof v.gameMode === 'string' && uint(v.mapId) &&
    (v.map === null || typeof v.map === 'string') && (v.pickType === null || typeof v.pickType === 'string') &&
    (v.spectators === null || typeof v.spectators === 'string');
}
// Personalizadas abertas (#264).
function customGameListing(v: unknown): v is CustomGameListing {
  return object(v) && ['partyId', 'name', 'owner'].every(k => typeof v[k] === 'string') &&
    ['mapId', 'players', 'maxPlayers', 'spectators', 'maxSpectators'].every(k => uint(v[k])) &&
    nullableString(v.map) && nullableString(v.spectatorPolicy) && typeof v.hasPassword === 'boolean';
}
const customGames = (v: unknown): v is CustomGameListing[] | null => v === null || list(v, customGameListing);
function customLobby(v: unknown): v is CustomLobby {
  return object(v) && customGameSettings(v.settings) && uint(v.teamSize) && Array.isArray(v.team1) && v.team1.every(customMember) &&
    Array.isArray(v.team2) && v.team2.every(customMember) &&
    Array.isArray(v.spectators) && v.spectators.every(customMember) && uint(v.maxSpectators);
}
function lobby(v: unknown): v is Lobby | null {
  return v === null || (object(v) && uint(v.queueId) && (v.queue === null || isQueue(v.queue)) &&
    uint(v.maxLobbySize) && typeof v.isLeader === 'boolean' && typeof v.canStartActivity === 'boolean' &&
    typeof v.showPositionSelector === 'boolean' && typeof v.showQuickPlaySlots === 'boolean' &&
    Array.isArray(v.scarcePositions) && v.scarcePositions.every(position) && Array.isArray(v.members) && v.members.every(lobbyMember) &&
    typeof v.canInvite === 'boolean' && Array.isArray(v.invitations) && v.invitations.every(lobbyInvitation) &&
    typeof v.isOpen === 'boolean' && Array.isArray(v.restrictions) && v.restrictions.every(lobbyRestriction) &&
    (v.custom === null || customLobby(v.custom)));
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
const readyCheckStates = {
  InProgress: true, EveryoneReady: true, StrangerNotReady: true, PartyNotReady: true,
  Error: true, Invalid: true, Unknown: true,
} satisfies Record<ReadyCheckState, true>;
const readyCheckResponses = { None: true, Accepted: true, Declined: true, Unknown: true } satisfies Record<ReadyCheckResponse, true>;
function readyCheck(v: unknown): v is ReadyCheck | null {
  return v === null || (object(v) && typeof v.state === 'string' && Object.hasOwn(readyCheckStates, v.state) &&
    typeof v.playerResponse === 'string' && Object.hasOwn(readyCheckResponses, v.playerResponse) && nums(v, ['timer', 'duration']));
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
function gameQueue(v: unknown): v is GameQueue {
  return object(v) && uints(v, ['queueId', 'mapId']) && (v.queue === null || isQueue(v.queue)) &&
    strings(v, ['name', 'description', 'gameMode', 'modeGroup']) &&
    booleans(v, ['versusAi', 'ranked', 'showPositionSelector', 'limitedTime', 'available']);
}
function champSelect(v: unknown): v is ChampSelect | null {
  return v === null || (object(v) && typeof v.id === 'string' && uint(v.queueId) && Number.isSafeInteger(v.localPlayerCellId) &&
    (v.queue === null || isQueue(v.queue)) && booleans(v, ['isCustomGame', 'hasSimultaneousBans', 'hasSimultaneousPicks']) &&
    object(v.timer) && typeof v.timer.phase === 'string' && object(v.bans) && arrays(v.bans, ['myTeam', 'theirTeam']) &&
    arrays(v, ['myTeam', 'theirTeam', 'actions']) && (v.pendingAction === null || object(v.pendingAction)));
}
function champSelectChat(v: unknown): v is ChampSelectChat | null {
  return v === null || (object(v) && list(v.messages, (m): m is ChampSelectChatMessage => object(m) &&
    strings(m, ['id', 'body', 'timestamp']) && typeof m.fromMe === 'boolean' &&
    (m.fromCellId === null || (typeof m.fromCellId === 'number' && Number.isSafeInteger(m.fromCellId)))));
}
function lobbyChat(v: unknown): v is LobbyChat | null {
  return v === null || (object(v) && list(v.messages, (m): m is LobbyChatMessage => object(m) &&
    strings(m, ['id', 'body', 'fromPuuid', 'gameName', 'tagLine', 'timestamp']) && typeof m.fromMe === 'boolean' &&
    (m.kind === 'chat' || m.kind === 'joined' || m.kind === 'left')));
}
function receivedInvitation(v: unknown): v is ReceivedInvitation {
  return object(v) && strings(v, ['invitationId', 'fromSummonerName']) && uints(v, ['fromSummonerId', 'queueId']) &&
    (v.queue === null || isQueue(v.queue)) && typeof v.canAccept === 'boolean';
}
function conversation(v: unknown): v is Conversation {
  return object(v) && strings(v, ['id', 'gameName', 'tagLine']) && uint(v.unreadCount) && list(v.messages, object);
}
function runePages(v: unknown): v is RunePages | null {
  return v === null || (object(v) && list(v.pages, object) && (v.currentPageId === null || uint(v.currentPageId)) &&
    typeof v.canCreatePage === 'boolean');
}
function runeCatalog(v: unknown): v is RuneCatalog | null {
  return v === null || (object(v) && list(v.styles, object) && list(v.perks, object));
}
function summonerSpellCatalog(v: unknown): v is SummonerSpellCatalog | null {
  return v === null || (object(v) && list(v.spells, object));
}
function championCatalog(v: unknown): v is ChampionCatalog | null {
  return v === null || (object(v) && list(v.champions, object));
}
const nullableIds = (v: unknown) => v === null || list(v, uint);
function tutorialPart(v: unknown): v is TutorialPart {
  return object(v) && uint(v.queueId) && uint(v.part) && typeof v.completed === 'boolean' && typeof v.available === 'boolean';
}
const tutorial = (v: unknown) => v === null || list(v, tutorialPart);
function champSelectChampions(v: unknown): v is ChampSelectChampions {
  return object(v) && nullableIds(v.pickable) && nullableIds(v.bannable);
}
function friend(v: unknown): v is Friend {
  return object(v) && uint(v.summonerId) && uint(v.groupId) &&
    strings(v, ['id', 'puuid', 'gameName', 'tagLine', 'name', 'availability', 'statusMessage', 'groupName']) &&
    (v.profileIconId === null || uint(v.profileIconId)) && (v.activity === null || object(v.activity));
}
function friendGroup(v: unknown): v is FriendGroup {
  return object(v) && uint(v.id) && typeof v.name === 'string' && Number.isInteger(v.priority) && typeof v.collapsed === 'boolean';
}
function recentPlayer(v: unknown): v is RecentPlayer {
  return object(v) && uint(v.summonerId) && strings(v, ['puuid', 'gameName', 'tagLine', 'summonerName', 'playedAt']) &&
    (v.championId === null || uint(v.championId));
}
// Perfil (#171, #172, #174, #175).
function rankedEntry(v: unknown): v is RankedEntry {
  return object(v) && nullableString(v.tier) && nullableString(v.division) &&
    uints(v, ['leaguePoints', 'wins', 'losses']) && typeof v.provisional === 'boolean';
}
function ranked(v: unknown): v is RankedStats | null {
  return v === null || (object(v) && rankedEntry(v.soloDuo) && rankedEntry(v.flex));
}
function championMastery(v: unknown): v is ChampionMastery {
  return object(v) && uints(v, ['championId', 'level', 'points']);
}
const matchResults = { win: true, loss: true, remake: true } satisfies Record<MatchResult, true>;
function matchPlayer(v: unknown): v is MatchPlayer {
  return object(v) && strings(v, ['puuid', 'gameName', 'tagLine']) &&
    uints(v, ['profileIconId', 'teamId', 'championId', 'championLevel', 'spell1Id', 'spell2Id', 'kills', 'deaths',
      'assists', 'cs', 'gold', 'damageToChampions', 'visionScore', 'perkId', 'perkStyleId', 'perkSubStyleId']) &&
    typeof v.win === 'boolean' && list(v.items, uint);
}
function matchInfo(v: Record<string, unknown>): boolean {
  return uints(v, ['gameId', 'queueId', 'mapId', 'createdAt', 'durationSecs']) && typeof v.gameMode === 'string';
}
function matchSummary(v: unknown): v is MatchSummary {
  return object(v) && matchInfo(v) && typeof v.result === 'string' && Object.hasOwn(matchResults, v.result) &&
    matchPlayer(v.player);
}
function matchHistory(v: unknown): v is MatchHistory | null {
  return v === null || (object(v) && list(v.matches, matchSummary) && typeof v.hasMore === 'boolean');
}
function matchDetails(v: unknown): v is MatchDetails | null {
  return v === null || (object(v) && matchInfo(v) && typeof v.remake === 'boolean' && list(v.players, matchPlayer));
}
// Perfil de outro jogador (#255).
function playerProfile(v: unknown): v is PlayerProfile | null {
  return v === null || (object(v) && v.summoner !== null && summoner(v.summoner) && typeof v.privateProfile === 'boolean' &&
    ranked(v.ranked) && list(v.championMasteries, championMastery) && matchHistory(v.matchHistory));
}
// Notas da atualização do Início (#301).
function patchNotes(v: unknown): v is PatchNotes | null {
  return v === null || (object(v) && typeof v.version === 'string' && typeof v.url === 'string' &&
    nullableString(v.title) && nullableString(v.summary));
}
// Partida ao vivo (#268).
function tierDivision(v: unknown): boolean {
  return v === null || (object(v) && typeof v.tier === 'string' && nullableString(v.division));
}
function liveRunes(v: unknown): boolean {
  return v === null || (object(v) && uints(v, ['keystoneId', 'primaryStyleId', 'subStyleId']));
}
function livePlayerStats(v: unknown): boolean {
  return v === null || (object(v) && (v.rank === null || rankedEntry(v.rank)) && tierDivision(v.previousSeason) &&
    uints(v, ['recentGames', 'championGames', 'championWins']) && position(v.mainPosition) &&
    typeof v.streak === 'number' && Number.isSafeInteger(v.streak));
}
function livePlayer(v: unknown): boolean {
  return object(v) && strings(v, ['puuid', 'gameName', 'tagLine']) && booleans(v, ['me', 'bot', 'hidden']) &&
    uints(v, ['championId', 'spell1Id', 'spell2Id']) && position(v.position) && liveRunes(v.runes) && livePlayerStats(v.stats);
}
function liveTeam(v: unknown): boolean {
  return object(v) && uint(v.teamId) && tierDivision(v.averageTier) && Array.isArray(v.players) && v.players.every(livePlayer);
}
function liveGame(v: unknown): v is LiveGame | null {
  return v === null || (object(v) && uints(v, ['gameId', 'queueId', 'mapId']) && strings(v, ['queueName', 'mapName', 'gameMode']) &&
    (v.rankedQueue === 'soloDuo' || v.rankedQueue === 'flex') && (v.gameTimeSecs === null || uint(v.gameTimeSecs)) &&
    Array.isArray(v.teams) && v.teams.every(liveTeam));
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
function lobbyLink(v: unknown): v is LobbyLink | null {
  return v === null || (object(v) && typeof v.url === 'string' && typeof v.joinCode === 'string' &&
    (v.expiresAt === null || uint(v.expiresAt)));
}
function state(v: unknown): v is AgentState {
  return object(v) && status(v.lcu) && summoner(v.summoner) && nullableString(v.gameflowPhase) && lobby(v.lobby) && lobbyLink(v.lobbyLink) &&
    readyCheck(v.readyCheck) && matchmaking(v.matchmaking) && list(v.queues, gameQueue) && nullableIds(v.playableChampions) && tutorial(v.tutorial) && nullableIds(v.availableBots) && lobbyChat(v.lobbyChat) && customGames(v.customGames) && champSelect(v.champSelect) &&
    list(v.receivedInvitations, receivedInvitation) && list(v.conversations, conversation) &&
    champSelectChat(v.champSelectChat) && runePages(v.runePages) && runeCatalog(v.runeCatalog) &&
    summonerSpellCatalog(v.summonerSpellCatalog) && championCatalog(v.championCatalog) &&
    champSelectChampions(v.champSelectChampions) && list(v.friends, friend) && list(v.friendGroups, friendGroup) && list(v.recentPlayers, recentPlayer) &&
    ranked(v.ranked) && list(v.championMasteries, championMastery) && matchHistory(v.matchHistory) &&
    matchDetails(v.matchDetails) && playerProfile(v.playerProfile) && liveGame(v.liveGame) && patchNotes(v.patchNotes);
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
    case 'queues': valid = list(v.queues, gameQueue); break;
    case 'playableChampions': valid = nullableIds(v.playableChampions); break;
    case 'tutorial': valid = tutorial(v.tutorial); break;
    case 'availableBots': valid = nullableIds(v.availableBots); break;
    case 'customGames': valid = customGames(v.customGames); break;
    case 'champSelect': valid = champSelect(v.champSelect); break;
    case 'champSelectChat': valid = champSelectChat(v.champSelectChat); break;
    case 'lobbyChat': valid = lobbyChat(v.lobbyChat); break;
    case 'lobbyLink': valid = lobbyLink(v.lobbyLink); break;
    case 'receivedInvitations': valid = list(v.invitations, receivedInvitation); break;
    case 'conversation': valid = conversation(v.conversation); break;
    case 'conversationClosed': valid = typeof v.id === 'string'; break;
    case 'runePages': valid = runePages(v.runePages); break;
    case 'runeCatalog': valid = runeCatalog(v.runeCatalog); break;
    case 'summonerSpellCatalog': valid = summonerSpellCatalog(v.summonerSpellCatalog); break;
    case 'championCatalog': valid = championCatalog(v.championCatalog); break;
    case 'champSelectChampions': valid = champSelectChampions(v.champSelectChampions); break;
    case 'friends': valid = list(v.friends, friend); break;
    case 'friendGroups': valid = list(v.friendGroups, friendGroup); break;
    case 'recentPlayers': valid = list(v.recentPlayers, recentPlayer); break;
    case 'ranked': valid = ranked(v.ranked); break;
    case 'championMasteries': valid = list(v.championMasteries, championMastery); break;
    case 'matchHistory': valid = matchHistory(v.matchHistory); break;
    case 'matchDetails': valid = matchDetails(v.matchDetails); break;
    case 'playerProfile': valid = playerProfile(v.playerProfile); break;
    case 'liveGame': valid = liveGame(v.liveGame); break;
    case 'patchNotes': valid = patchNotes(v.patchNotes); break;
    case 'response': valid = uint(v.id) && object(v.result) && (v.result.status === 'ok' ||
      (v.result.status === 'error' && errorCode(v.result.code) && typeof v.result.message === 'string')); break;
    case 'invalidMessage': valid = typeof v.message === 'string'; break;
    default: return null;
  }
  if (!valid) throw new Error('Mensagem inválida do agente.');
  return v as ServerMessage;
}

import type { ChampSelect } from '../bindings/ChampSelect';
import type { Conversation } from '../bindings/Conversation';
import type { Friend } from '../bindings/Friend';
import type { Matchmaking } from '../bindings/Matchmaking';
import type { ReadyCheck } from '../bindings/ReadyCheck';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { RunePages } from '../bindings/RunePages';
import type { SummonerSpellCatalog } from '../bindings/SummonerSpellCatalog';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';

export const testReadyCheck: ReadyCheck = { state: 'InProgress', playerResponse: 'None', timer: 2.5, duration: 12 };
export const testMatchmaking: Matchmaking = {
  searchState: 'Searching', queueId: 420, timeInQueue: 31.4, estimatedQueueTime: 95.2,
  lowPriorityTimeRemaining: 0, errors: [],
};
export const testChampSelect: ChampSelect = {
  id: 'sessao-1', queueId: 420, queue: 'rankedSolo', isCustomGame: false, localPlayerCellId: 0,
  hasSimultaneousBans: true, hasSimultaneousPicks: false,
  timer: { phase: 'BAN_PICK', timeLeftMs: 30000, totalTimeMs: 30000, isInfinite: false, endsAtMs: 1_800_000_000_000 },
  myTeam: [{ cellId: 0, isLocalPlayer: true, gameName: 'Victor', tagLine: 'BR1', assignedPosition: 'middle', isAutofilled: false,
    championId: null, championPickIntent: null, selectedSkinId: null, spell1Id: 4, spell2Id: 14 }],
  theirTeam: [], bans: { myTeam: [], theirTeam: [] },
  actions: [{ id: 1, type: 'ban', actorCellId: 0, championId: null, completed: false, isInProgress: true, isAllyAction: true, turn: 1 }],
  pendingAction: null,
  trades: [],
  pickOrderSwaps: [],
  positionSwaps: [],
  benchEnabled: false,
  benchChampions: [],
  allowRerolling: false,
  rerollsRemaining: 0,
  autofillNotice: false,
};
export const testInvitation: ReceivedInvitation = {
  invitationId: 'convite-1', fromSummonerId: 2, fromSummonerName: '', queueId: 420, queue: 'rankedSolo', canAccept: true,
};
export const testConversation: Conversation = {
  id: 'amigo@br1.pvp.net', gameName: 'Amigo', tagLine: 'BR1', unreadCount: 1,
  messages: [{ id: 'm1', body: 'bora?', fromMe: false, timestamp: '2026-11-05T01:13:41.755Z' }],
};
export const testRunePages: RunePages = {
  pages: [{ id: 10, name: 'Mid', primaryStyleId: 8100, subStyleId: 8300, selectedPerkIds: [8112], isEditable: true,
    isDeletable: true, isTemporary: false, isValid: true }],
  currentPageId: 10, canCreatePage: true,
};
export const testRuneCatalog: RuneCatalog = {
  styles: [{ id: 8100, name: 'Dominação', allowedSubStyles: [8300], slots: [] }],
  perks: [{ id: 8112, name: 'Eletrocutar', shortDesc: 'Dano extra' }],
};
export const testSummonerSpellCatalog: SummonerSpellCatalog = {
  spells: [{ id: 4, name: 'Flash', description: 'Teleporta.', summonerLevel: 7, gameModes: ['CLASSIC'], queues: ['rankedSolo'] }],
};
export const testFriend: Friend = {
  id: 'amigo@br1.pvp.net', puuid: 'amigo', summonerId: 1003, gameName: 'Amigo', tagLine: 'BR1', name: '', profileIconId: null,
  availability: 'online', statusMessage: '', groupId: 0, groupName: '**Default', activity: null,
};
export const testChampionCatalog: ChampionCatalog = {
  champions: [
    { id: 266, name: 'Aatrox', alias: 'Aatrox', roles: ['fighter'], positions: ['top'] },
    { id: 103, name: 'Ahri', alias: 'Ahri', roles: ['mage'], positions: ['middle'] },
    { id: 145, name: "Kai'Sa", alias: 'Kaisa', roles: ['marksman'], positions: ['bottom'] },
    { id: 20, name: 'Nunu e Willump', alias: 'Nunu', roles: ['tank'], positions: ['jungle', 'utility'] },
    { id: 62, name: 'Wukong', alias: 'MonkeyKing', roles: ['fighter'], positions: ['jungle', 'top'] },
  ],
};

import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectPlayer } from '../bindings/ChampSelectPlayer';
import { testChampionCatalog } from './events';
import { testState } from './lobby';

const player = (cellId: number, change: Partial<ChampSelectPlayer> = {}): ChampSelectPlayer => ({
  cellId, isLocalPlayer: false, gameName: null, tagLine: null, assignedPosition: 'unselected', championId: null,
  championPickIntent: null, selectedSkinId: null, spell1Id: null, spell2Id: null, ...change,
});

/** Seleção ranqueada na vez de banir do jogador logado (célula 0). */
export const draftChampSelect: ChampSelect = {
  id: 'sessao-draft', queueId: 420, queue: 'rankedSolo', isCustomGame: false, localPlayerCellId: 0,
  hasSimultaneousBans: true, hasSimultaneousPicks: false,
  timer: { phase: 'BAN_PICK', timeLeftMs: 30000, totalTimeMs: 30000, isInfinite: false, endsAtMs: Date.UTC(2026, 10, 6, 12, 0, 30) },
  myTeam: [
    player(0, { isLocalPlayer: true, gameName: 'Victor', tagLine: 'BR1', assignedPosition: 'middle', spell1Id: 4, spell2Id: 14 }),
    player(1, { gameName: 'Amigo', tagLine: 'BR1', assignedPosition: 'jungle', championPickIntent: 62 }),
  ],
  theirTeam: [player(5), player(6, { championId: 103 })],
  bans: { myTeam: [], theirTeam: [] },
  actions: [
    { id: 1, type: 'ban', actorCellId: 0, championId: null, completed: false, isInProgress: true, isAllyAction: true, turn: 0 },
    { id: 2, type: 'ban', actorCellId: 1, championId: null, completed: false, isInProgress: true, isAllyAction: true, turn: 0 },
    { id: 3, type: 'pick', actorCellId: 6, championId: 103, completed: true, isInProgress: false, isAllyAction: false, turn: 1 },
    { id: 4, type: 'pick', actorCellId: 0, championId: null, completed: false, isInProgress: false, isAllyAction: true, turn: 2 },
  ],
  pendingAction: { id: 1, type: 'ban', actorCellId: 0, championId: null, completed: false, isInProgress: true, isAllyAction: true, turn: 0 },
  trades: [],
  pickOrderSwaps: [],
  positionSwaps: [],
};

/** Estado do agente durante a seleção, com o catálogo de campeões. */
export const champSelectState: AgentState = {
  ...testState,
  gameflowPhase: 'ChampSelect',
  champSelect: draftChampSelect,
  championCatalog: testChampionCatalog,
  champSelectChampions: { pickable: [20, 62, 103, 266], bannable: [20, 62, 103, 145, 266] },
};

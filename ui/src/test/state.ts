import type { AgentState } from '../bindings/AgentState';

/**
 * Estado de um agente conectado ao LoL, sem nada aberto: sem invocador, sem
 * lobby, sem seleção e com as listas vazias.
 */
export const emptyState: AgentState = {
  lcu: { state: 'connected', session: 1 },
  summoner: null,
  gameflowPhase: null,
  lobby: null,
  matchmaking: null,
  readyCheck: null,
  queues: [],
  playableChampions: null,
  champSelect: null,
  champSelectChat: null,
  lobbyChat: null,
  summonerSpellCatalog: null,
  championCatalog: null,
  champSelectChampions: { pickable: null, bannable: null, subset: null },
  runePages: null,
  runeCatalog: null,
  friends: [],
  friendGroups: [],
  recentPlayers: [],
  receivedInvitations: [],
  conversations: [],
  ranked: null,
  championMasteries: [],
  matchHistory: null,
  matchDetails: null,
};

/**
 * Monta um `AgentState` para os testes: o `emptyState` com só os campos que
 * o teste quer mudar. Quando o agente ganha um campo novo, só o
 * `emptyState` muda, e os testes que não usam o campo ficam como estão.
 */
export function makeState(overrides: Partial<AgentState> = {}): AgentState {
  return { ...emptyState, ...overrides };
}

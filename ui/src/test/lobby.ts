import type { AgentState } from '../bindings/AgentState';
import type { Lobby } from '../bindings/Lobby';

export const testLobby: Lobby = {
  queueId: 420, queue: 'rankedSolo', maxLobbySize: 2, isLeader: true,
  canStartActivity: true, showPositionSelector: true, canInvite: true, invitations: [],
  members: [{ puuid: 'local', summonerId: 1, summonerIconId: 1, summonerLevel: 122, isLeader: true,
    firstPosition: 'unselected', secondPosition: 'unselected' }],
};
export const testState: AgentState = {
  lcu: { state: 'connected', session: 1 }, gameflowPhase: 'None', lobby: null, readyCheck: null, matchmaking: null, champSelect: null, conversations: [], friends: [], runeCatalog: null, runePages: null, receivedInvitations: [],
  summoner: { gameName: 'Victor', tagLine: 'BR1', displayName: 'Victor', puuid: 'local', summonerId: 1,
    profileIconId: 1, summonerLevel: 122 },
};

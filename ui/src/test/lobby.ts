import type { AgentState } from '../bindings/AgentState';
import type { Lobby } from '../bindings/Lobby';
import { makeState } from './state';

export const testLobby: Lobby = {
  queueId: 420, queue: 'rankedSolo', maxLobbySize: 2, isLeader: true,
  canStartActivity: true, showPositionSelector: true, showQuickPlaySlots: false, scarcePositions: [], canInvite: true, invitations: [], isOpen: false, restrictions: [],
  members: [{ puuid: 'local', summonerId: 1, summonerIconId: 1, summonerLevel: 122, isLeader: true,
    firstPosition: 'unselected', secondPosition: 'unselected', playerSlots: [] }],
};
export const testState: AgentState = makeState({ gameflowPhase: 'None', summoner: { gameName: 'Victor', tagLine: 'BR1', displayName: 'Victor', puuid: 'local', summonerId: 1, profileIconId: 1, summonerLevel: 122 } });

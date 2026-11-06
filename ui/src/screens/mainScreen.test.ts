import { describe, expect, it } from 'vitest';
import { mainScreen, phaseLabel } from './mainScreen';
import { testState, testLobby } from '../test/lobby';
import { testMatchmaking } from '../test/events';
import type { AgentState } from '../bindings/AgentState';
import type { GameflowPhase } from '../bindings/GameflowPhase';

const select = (state: Partial<AgentState>) => mainScreen({ status: 'connected', error: null, state: { ...testState, ...state } });
describe('prioridade das telas', () => {
  it('aguarda login e conexão mesmo com dados antigos', () => {
    expect(select({ summoner: null, lobby: testLobby })).toBe('home');
    expect(select({ lcu: { state: 'starting' }, matchmaking: testMatchmaking })).toBe('home');
    expect(select({ gameflowPhase: null })).toBe('home');
  });
  it('considera tanto fase quanto os eventos de lobby/fila', () => {
    expect(select({ lobby: testLobby })).toBe('lobby');
    expect(select({ gameflowPhase: 'Lobby' })).toBe('lobby');
    expect(select({ matchmaking: testMatchmaking })).toBe('queue');
    expect(select({ gameflowPhase: 'Matchmaking' })).toBe('queue');
    expect(select({ gameflowPhase: 'ReadyCheck' })).toBe('found');
    expect(select({ readyCheck: { state: 'InProgress', playerResponse: 'None', timer: 0, duration: 10 } })).toBe('found');
  });
  it('mostra a seleção de campeões na fase dela, acima de lobby/fila antigos', () => {
    expect(select({ gameflowPhase: 'ChampSelect', lobby: testLobby, matchmaking: testMatchmaking })).toBe('champSelect');
  });
  it.each<GameflowPhase>(['GameStart', 'InProgress', 'Reconnect', 'FailedToLaunch', 'TerminatedInError', 'CheckedIntoTournament', 'WaitingForStats', 'PreEndOfGame', 'EndOfGame', 'Unknown'])('prioriza %s sobre lobby/fila antigos', phase => {
    expect(select({ gameflowPhase: phase, lobby: testLobby, matchmaking: testMatchmaking })).toBe('other');
    expect(phaseLabel(phase)).toBeTruthy();
  });
});

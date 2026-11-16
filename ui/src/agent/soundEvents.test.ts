import { describe, expect, it } from 'vitest';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import { testChampSelect, testConversation, testInvitation, testReadyCheck } from '../test/events';
import { testLobby, testState } from '../test/lobby';
import { soundCues, type SoundContext } from './soundEvents';

const away: SoundContext = { focused: false, openConversationId: null };
const state = (overrides: Partial<AgentState>): AgentState => ({ ...testState, ...overrides });
const inChampSelect = (champSelect: Partial<ChampSelect>) =>
  state({ gameflowPhase: 'ChampSelect', champSelect: { ...testChampSelect, ...champSelect } });
const myBan = { ...testChampSelect.actions[0] };
const allyPick = { id: 7, type: 'pick' as const, actorCellId: 1, championId: 266, completed: false, isInProgress: true, isAllyAction: true, turn: 2 };

describe('soundCues', () => {
  it('não toca nada sem estado anterior ou com outra sessão do LoL', () => {
    const next = state({ receivedInvitations: [testInvitation] });
    expect(soundCues(null, next, away)).toEqual([]);
    expect(soundCues(state({ lcu: { state: 'connected', session: 9 } }), next, away)).toEqual([]);
  });

  it('toca ao entrar na seleção, sem repetir o aviso da vez', () => {
    const prev = state({ gameflowPhase: 'ReadyCheck', readyCheck: testReadyCheck });
    expect(soundCues(prev, inChampSelect({ pendingAction: myBan }), away)).toEqual(['champSelectStart']);
  });

  it('avisa quando começa a sua vez, uma vez por ação', () => {
    const waiting = inChampSelect({});
    const myTurn = inChampSelect({ pendingAction: myBan });
    expect(soundCues(waiting, myTurn, away)).toEqual(['turnStart']);
    expect(soundCues(myTurn, inChampSelect({ pendingAction: myBan, timer: { ...testChampSelect.timer, timeLeftMs: 5 } }), away)).toEqual([]);
  });

  it('avisa a partida cancelada no aceite e no dodge, mas não quem recusou', () => {
    const readyCheck = state({ gameflowPhase: 'ReadyCheck', readyCheck: testReadyCheck });
    expect(soundCues(readyCheck, state({ gameflowPhase: 'Matchmaking' }), away)).toEqual(['cancelled']);
    const declined = state({ gameflowPhase: 'ReadyCheck', readyCheck: { ...testReadyCheck, playerResponse: 'Declined' } });
    expect(soundCues(declined, state({ gameflowPhase: 'Lobby' }), away)).toEqual([]);
    expect(soundCues(inChampSelect({}), state({ gameflowPhase: 'Lobby', lobby: testLobby }), away)).toEqual(['cancelled']);
  });

  it('toca quando a seleção acaba e a partida carrega, uma vez', () => {
    const starting = inChampSelect({ timer: { ...testChampSelect.timer, phase: 'GAME_STARTING' } });
    expect(soundCues(inChampSelect({}), starting, away)).toEqual(['gameStart']);
    expect(soundCues(starting, state({ gameflowPhase: 'GameStart', champSelect: null }), away)).toEqual([]);
    expect(soundCues(inChampSelect({}), state({ gameflowPhase: 'GameStart', champSelect: null }), away)).toEqual(['gameStart']);
  });

  it('avisa o pedido de troca recebido e a escolha confirmada de um aliado', () => {
    const swap = { id: 3, cellId: 1, state: 'AVAILABLE' as const };
    const before = inChampSelect({ trades: [swap], actions: [myBan, allyPick] });
    expect(soundCues(before, inChampSelect({ trades: [{ ...swap, state: 'RECEIVED' }], actions: [myBan, allyPick] }), away))
      .toEqual(['swapRequest']);
    expect(soundCues(before, inChampSelect({ trades: [swap], actions: [myBan, { ...allyPick, completed: true }] }), away))
      .toEqual(['allyLockIn']);
    // A própria escolha toca em useLockInSound.
    const mine = { ...allyPick, actorCellId: 0 };
    expect(soundCues(inChampSelect({ actions: [mine] }), inChampSelect({ actions: [{ ...mine, completed: true }] }), away)).toEqual([]);
  });

  it('avisa convite novo', () => {
    expect(soundCues(testState, state({ receivedInvitations: [testInvitation] }), away)).toEqual(['invite']);
    const invited = state({ receivedInvitations: [testInvitation] });
    expect(soundCues(invited, state({ receivedInvitations: [{ ...testInvitation }] }), away)).toEqual([]);
  });

  it('avisa mensagem recebida, menos na conversa aberta com a janela em foco', () => {
    const prev = state({ conversations: [testConversation] });
    const incoming = { id: 'm2', body: 'oi', fromMe: false, timestamp: '2026-11-05T01:14:00.000Z' };
    const next = state({ conversations: [{ ...testConversation, messages: [...testConversation.messages, incoming] }] });
    expect(soundCues(prev, next, away)).toEqual(['message']);
    expect(soundCues(prev, next, { focused: true, openConversationId: testConversation.id })).toEqual([]);
    expect(soundCues(prev, next, { focused: false, openConversationId: testConversation.id })).toEqual(['message']);
    const mine = state({ conversations: [{ ...testConversation, messages: [...testConversation.messages, { ...incoming, fromMe: true }] }] });
    expect(soundCues(prev, mine, away)).toEqual([]);
  });

  it('avisa mensagem no chat da seleção', () => {
    const message = { id: 'c1', body: 'mid', fromMe: false, fromCellId: 1, timestamp: '2026-11-05T01:14:00.000Z' };
    const prev = { ...inChampSelect({}), champSelectChat: { messages: [] } };
    expect(soundCues(prev, { ...inChampSelect({}), champSelectChat: { messages: [message] } }, away)).toEqual(['message']);
    expect(soundCues(prev, { ...inChampSelect({}), champSelectChat: { messages: [{ ...message, fromMe: true }] } }, away)).toEqual([]);
  });

  it('avisa quem entrou e quem saiu do lobby', () => {
    const friend = { ...testLobby.members[0], puuid: 'amigo', isLeader: false };
    const alone = state({ gameflowPhase: 'Lobby', lobby: testLobby });
    const together = state({ gameflowPhase: 'Lobby', lobby: { ...testLobby, members: [...testLobby.members, friend] } });
    expect(soundCues(alone, together, away)).toEqual(['memberJoin']);
    expect(soundCues(together, alone, away)).toEqual(['memberLeave']);
    expect(soundCues(state({ gameflowPhase: 'None' }), together, away)).toEqual([]);
  });
});

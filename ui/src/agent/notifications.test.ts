import { describe, expect, it } from 'vitest';
import type { AgentState } from '../bindings/AgentState';
import type { ChatMessage } from '../bindings/ChatMessage';
import type { Conversation } from '../bindings/Conversation';
import { testConversation, testInvitation, testReadyCheck } from '../test/events';
import { makeState } from '../test/state';
import { conversationName, invitationText, matchFoundPending, updateMessageNotices } from './notifications';

const message = (id: string, body = id, fromMe = false): ChatMessage => ({ id, body, fromMe, timestamp: '2026-11-05T01:13:41.755Z' });
const conversation = (messages: ChatMessage[], change: Partial<Conversation> = {}): Conversation =>
  ({ ...testConversation, messages, ...change });
const withChat = (...conversations: Conversation[]): AgentState => makeState({ conversations });

describe('updateMessageNotices', () => {
  const before = withChat(conversation([message('m1')]));

  it('não avisa o histórico do primeiro estado nem de outra sessão', () => {
    expect(updateMessageNotices([], null, before)).toEqual([]);
    const other = { ...withChat(conversation([message('m1'), message('m2')])), lcu: { state: 'connected', session: 2 } } as AgentState;
    expect(updateMessageNotices([], before, other)).toEqual([]);
  });

  it('avisa mensagem recebida e soma as seguintes no mesmo aviso', () => {
    const one = withChat(conversation([message('m1'), message('m2', 'oi')]));
    const notices = updateMessageNotices([], before, one);
    expect(notices).toEqual([{ conversationId: testConversation.id, from: 'Amigo#BR1', body: 'oi', count: 1, lastMessageId: 'm2' }]);
    const two = withChat(conversation([message('m1'), message('m2', 'oi'), message('m3', 'bora'), message('m4', 'ranked?')]));
    expect(updateMessageNotices(notices, one, two)).toEqual([{ ...notices[0], body: 'ranked?', count: 3, lastMessageId: 'm4' }]);
  });

  it('ignora mensagem minha e conversa já lida', () => {
    expect(updateMessageNotices([], before, withChat(conversation([message('m1'), message('m2', 'x', true)])))).toEqual([]);
    expect(updateMessageNotices([], before, withChat(conversation([message('m1'), message('m2')], { unreadCount: 0 })))).toEqual([]);
  });

  it('avisa conversa nova e tira o aviso quando ela é lida ou fechada', () => {
    const nova = conversation([message('n1', 'e aí')], { id: 'outro', gameName: 'Outro', tagLine: '' });
    const after = withChat(conversation([message('m1')]), nova);
    const notices = updateMessageNotices([], before, after);
    expect(notices).toMatchObject([{ conversationId: 'outro', from: 'Outro', body: 'e aí' }]);
    const read = withChat(conversation([message('m1')]), { ...nova, unreadCount: 0 });
    expect(updateMessageNotices(notices, after, read)).toEqual([]);
    expect(updateMessageNotices(notices, after, before)).toEqual([]);
  });

  it('limpa os avisos quando o agente ou o LoL desconecta', () => {
    const notices = updateMessageNotices([], before, withChat(conversation([message('m1'), message('m2')])));
    expect(updateMessageNotices(notices, before, null)).toEqual([]);
  });

  it('mantém a mesma lista quando nada muda', () => {
    const notices = updateMessageNotices([], before, withChat(conversation([message('m1'), message('m2')])));
    const state = withChat(conversation([message('m1'), message('m2')]));
    expect(updateMessageNotices(notices, state, { ...state, gameflowPhase: 'Lobby' })).toBe(notices);
  });
});

it('monta os textos de nome e convite', () => {
  expect(conversationName({ ...testConversation, gameName: '' })).toBe('Um amigo');
  expect(invitationText(testInvitation)).toBe('Um amigo convidou você para Ranqueada Solo/Duo');
  expect(invitationText({ ...testInvitation, fromSummonerName: 'Victor', queue: null })).toBe('Victor convidou você para um lobby');
});

it('só considera partida encontrada pendente antes de responder', () => {
  expect(matchFoundPending(makeState({ gameflowPhase: 'ReadyCheck', readyCheck: testReadyCheck }))).toBe(true);
  expect(matchFoundPending(makeState({ gameflowPhase: 'ReadyCheck', readyCheck: { ...testReadyCheck, playerResponse: 'Accepted' } }))).toBe(false);
  expect(matchFoundPending(makeState({ gameflowPhase: 'Lobby', readyCheck: testReadyCheck }))).toBe(false);
});

import { describe, expect, it } from 'vitest';
import type { Conversation } from '../bindings/Conversation';
import { testConversation, testFriend } from '../test/events';
import { conversationName, messageTime, sortConversations, totalUnread } from './chat';

const conversation = (overrides: Partial<Conversation>): Conversation => ({ ...testConversation, ...overrides });
const message = (timestamp: string) => ({ id: timestamp, body: 'oi', fromMe: false, timestamp });

describe('chat', () => {
  it('usa o nick da conversa (sem a tag) ou, sem ele, o nome da lista de amigos', () => {
    expect(conversationName(conversation({}), [])).toBe('Amigo');
    expect(conversationName(conversation({ tagLine: '' }), [])).toBe('Amigo');
    const antigo = { ...testFriend, gameName: '', tagLine: '', name: 'Antigo' };
    expect(conversationName(conversation({ gameName: '', tagLine: '' }), [antigo])).toBe('Antigo');
    expect(conversationName(conversation({ id: 'x', gameName: '', tagLine: '' }), [antigo])).toBe('Amigo');
  });

  it('ordena pela última mensagem e põe as conversas vazias no fim', () => {
    const velha = conversation({ id: 'a', gameName: 'Velha', messages: [message('2026-11-05T01:00:00Z')] });
    const nova = conversation({ id: 'b', gameName: 'Nova', messages: [message('2026-11-05T02:00:00Z')] });
    const vaziaB = conversation({ id: 'c', gameName: 'Bruno', messages: [] });
    const vaziaA = conversation({ id: 'd', gameName: 'Ana', messages: [] });
    expect(sortConversations([vaziaB, velha, vaziaA, nova], []).map(c => c.gameName)).toEqual(['Nova', 'Velha', 'Ana', 'Bruno']);
  });

  it('soma as não lidas', () => {
    expect(totalUnread([conversation({ unreadCount: 2 }), conversation({ unreadCount: 3 })])).toBe(5);
  });

  it('mostra a hora local da mensagem', () => {
    const date = new Date(2026, 10, 5, 9, 7);
    expect(messageTime(message(date.toISOString()))).toBe('09:07');
    expect(messageTime(message('quebrado'))).toBe('');
  });
});

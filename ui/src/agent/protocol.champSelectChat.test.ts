import { expect, it } from 'vitest';
import { parseMessage } from './protocol';
import { makeState } from '../test/state';

const message = { id: 'm1', body: 'oi', fromMe: false, fromCellId: null, timestamp: '2026-11-06T12:00:00Z' };

it.each([
  { id: 1 }, { body: {} }, { fromMe: 'false' }, { fromCellId: 0.5 }, { fromCellId: '1' }, { timestamp: null },
])('recusa mensagem de chat inválida em evento e hello: %j', invalid => {
  const champSelectChat = { messages: [{ ...message, ...invalid }] };
  expect(() => parseMessage(JSON.stringify({ type: 'champSelectChat', champSelectChat }))).toThrow();
  expect(() => parseMessage(JSON.stringify({ type: 'hello', protocol: 1, agentVersion: '0.1.0',
    state: makeState({ champSelectChat: champSelectChat as never }) }))).toThrow();
});

it('aceita sala ausente, vazia e células inteiras opcionais sem inventar ID de sala', () => {
  for (const champSelectChat of [null, { messages: [] }, { messages: [message, { ...message, id: 'm2', fromCellId: 0 }] }]) {
    const event = { type: 'champSelectChat', champSelectChat };
    expect(parseMessage(JSON.stringify(event))).toEqual(event);
  }
});

import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testConversation, testFriend } from '../test/events';
import { makeState } from '../test/state';

class Socket {
  static OPEN = 1;
  static instances: Socket[] = [];
  readyState = 1;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  send = vi.fn();
  close = vi.fn();
  constructor(readonly url: string) { Socket.instances.push(this); }
  receive(message: ServerMessage) { this.onmessage?.({ data: JSON.stringify(message) }); }
}
const socket = () => Socket.instances.at(-1)!;
const sent = (): ClientMessage[] => socket().send.mock.calls.map(([text]) => JSON.parse(text));

beforeEach(() => { Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.unstubAllGlobals(); });

it('abre o chat pela lista de amigos, manda a mensagem e mostra a conversa do agente', async () => {
  render(<App />);
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: makeState({ friends: [testFriend] }) }));
  fireEvent.click(screen.getByRole('button', { name: 'Conversar com Amigo#BR1' }));
  const view = within(screen.getByRole('region', { name: 'Conversa com Amigo#BR1' }));
  fireEvent.change(view.getByRole('textbox'), { target: { value: 'bora?' } });
  fireEvent.click(view.getByRole('button', { name: 'Enviar' }));
  const request = sent().at(-1)!;
  expect(request.command).toEqual({ type: 'sendChatMessage', conversationId: testFriend.id, body: 'bora?' });
  await act(async () => socket().receive({ type: 'response', id: request.id, result: { status: 'ok' } }));
  act(() => socket().receive({ type: 'conversation', conversation: { ...testConversation, unreadCount: 0,
    messages: [{ id: 'm1', body: 'bora?', fromMe: true, timestamp: '2026-11-05T01:13:41.755Z' }] } }));
  expect(view.getByRole('log')).toHaveTextContent('Você: bora?');
  expect(sent().at(-1)!.command).toEqual({ type: 'setActiveConversation', conversationId: testFriend.id });
});

import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelectChat } from '../bindings/ChampSelectChat';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { champSelectState } from '../test/champSelect';

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
const chat = () => within(screen.getByRole('region', { name: 'Chat do time' }));
const input = () => chat().getByRole('textbox', { name: 'Mensagem para o time' });
const last = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const room: ChampSelectChat = {
  messages: [
    { id: 'm1', body: 'vou de jungle', fromMe: false, fromCellId: 1, timestamp: '2026-11-05T01:13:41.755Z' },
    { id: 'm2', body: 'quem é?', fromMe: false, fromCellId: null, timestamp: '2026-11-05T01:14:00.000Z' },
  ],
};
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...champSelectState, champSelectChat: room, ...overrides } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: last().id, result }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('mostra as mensagens com quem mandou e recebe as novas', () => {
  render(<App />); hello();
  const log = within(chat().getByRole('log', { name: 'Mensagens do time' }));
  expect(log.getByText('vou de jungle').closest('li')).toHaveTextContent('Amigo#BR1: vou de jungle');
  expect(log.getByText('quem é?').closest('li')).toHaveTextContent('Aliado: quem é?');
  act(() => socket().receive({
    type: 'champSelectChat',
    champSelectChat: { messages: [...room.messages, { id: 'm3', body: 'gg', fromMe: true, fromCellId: 0, timestamp: '2026-11-05T01:15:00.000Z' }] },
  }));
  expect(log.getByText('gg').closest('li')).toHaveTextContent('Você: gg');
});

it('manda mensagem, limpa o campo e mostra erro sem perder o texto', async () => {
  render(<App />); hello();
  expect(chat().getByRole('button', { name: 'Enviar' })).toBeDisabled();
  fireEvent.change(input(), { target: { value: '  bora  ' } });
  fireEvent.submit(input());
  expect(last().command).toEqual({ type: 'sendChampSelectMessage', body: 'bora' });
  expect(chat().getByRole('button', { name: 'Enviando…' })).toBeDisabled();
  await respond();
  expect(input()).toHaveValue('');
  fireEvent.change(input(), { target: { value: 'de novo' } });
  fireEvent.submit(input());
  await respond({ status: 'error', code: 'lcu', message: 'sala fechada' });
  expect(chat().getByRole('alert')).toHaveTextContent('Não foi possível enviar a mensagem: sala fechada');
  expect(input()).toHaveValue('de novo');
});

it('espera a sala abrir', () => {
  render(<App />); hello({ champSelectChat: null });
  expect(chat().getByText('Aguardando a sala do chat abrir…')).toBeVisible();
  expect(input()).toBeDisabled();
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: { messages: [] } }));
  expect(chat().getByText('Nenhuma mensagem ainda.')).toBeVisible();
  expect(input()).toBeEnabled();
});

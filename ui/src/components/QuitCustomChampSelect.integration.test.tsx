import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { champSelectState, draftChampSelect } from '../test/champSelect';

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
const panel = () => within(screen.getByRole('region', { name: 'Seleção de campeões' }));
const sent = (): ClientMessage[] => socket().send.mock.calls.map(([text]) => JSON.parse(text as string));
const last = () => sent().at(-1)!;
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...champSelectState, ...overrides } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }, id = last().id) {
  await act(async () => socket().receive({ type: 'response', id, result }));
}
beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('fora da personalizada não mostra o botão Sair (seria esquiva)', () => {
  render(<App />); hello();
  expect(panel().queryByRole('button', { name: 'Sair' })).toBeNull();
});

it('na personalizada, Sair pede confirmação e manda quitCustomChampSelect', async () => {
  render(<App />); hello({ champSelect: { ...draftChampSelect, isCustomGame: true } });
  fireEvent.click(panel().getByRole('button', { name: 'Sair' }));
  expect(panel().getByText('Sair da seleção? Todos voltam para o lobby.')).toBeVisible();
  // Ficar volta ao botão sem mandar nada.
  const before = sent().length;
  fireEvent.click(panel().getByRole('button', { name: 'Ficar' }));
  expect(sent()).toHaveLength(before);
  fireEvent.click(panel().getByRole('button', { name: 'Sair' }));
  fireEvent.click(panel().getByRole('button', { name: 'Sair da seleção' }));
  expect(last().command).toEqual({ type: 'quitCustomChampSelect' });
  await respond({ status: 'error', code: 'lcu', message: 'recusado' });
  expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível sair da seleção: recusado');
  expect(panel().getByRole('button', { name: 'Sair' })).toBeVisible();
});

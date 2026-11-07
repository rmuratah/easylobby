import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { ChampSelect } from '../bindings/ChampSelect';
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
const last = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const withSwaps = (change: Partial<ChampSelect>): ChampSelect => ({ ...draftChampSelect, pendingAction: null, ...change });
function hello(champSelect: ChampSelect) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...champSelectState, champSelect } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: last().id, result }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('pede as trocas disponíveis com um aliado', async () => {
  render(<App />);
  hello(withSwaps({
    pickOrderSwaps: [{ id: 8, cellId: 1, state: 'AVAILABLE' }],
    positionSwaps: [{ id: 9, cellId: 1, state: 'AVAILABLE' }],
    trades: [{ id: 7, cellId: 1, state: 'INVALID' }],
  }));
  expect(panel().queryByRole('button', { name: /troca de campeão/ })).not.toBeInTheDocument();
  expect(screen.queryByRole('region', { name: 'Trocas' })).not.toBeInTheDocument();
  fireEvent.click(panel().getByRole('button', { name: 'Pedir troca de posição com Amigo#BR1' }));
  expect(last().command).toEqual({ type: 'requestSwap', kind: 'position', swapId: 9 });
  expect(panel().getByRole('button', { name: 'Pedir troca de vez com Amigo#BR1' })).toBeDisabled();
  await respond();
  act(() => socket().receive({
    type: 'champSelect',
    champSelect: withSwaps({ pickOrderSwaps: [{ id: 8, cellId: 1, state: 'INVALID' }], positionSwaps: [{ id: 9, cellId: 1, state: 'SENT' }] }),
  }));
  const sent = within(panel().getByRole('group', { name: 'Seu pedido de troca de posição' }));
  expect(sent.getByText('Aguardando Amigo#BR1 responder a troca de posição.')).toBeVisible();
  fireEvent.click(sent.getByRole('button', { name: 'Cancelar pedido' }));
  expect(last().command).toEqual({ type: 'cancelSwap', kind: 'position', swapId: 9 });
});

it.each([['Aceitar', 'acceptSwap'], ['Recusar', 'declineSwap']] as const)('%s um pedido recebido', async (label, type) => {
  render(<App />);
  hello(withSwaps({ trades: [{ id: 7, cellId: 1, state: 'RECEIVED' }] }));
  const request = within(panel().getByRole('group', { name: 'Pedido de troca de campeão' }));
  expect(request.getByText('Amigo#BR1 quer trocar de campeão com você.')).toBeVisible();
  fireEvent.click(request.getByRole('button', { name: label }));
  expect(last().command).toEqual({ type, kind: 'champion', swapId: 7 });
  fireEvent.click(request.getByRole('button', { name: label }));
  expect(socket().send).toHaveBeenCalledTimes(1);
  await respond({ status: 'error', code: 'invalidCommand', message: 'essa troca não está mais aberta' });
  expect(panel().getByRole('alert')).toHaveTextContent(`essa troca não está mais aberta`);
});

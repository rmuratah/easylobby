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

it('pede as trocas disponíveis com um aliado pelo botão Trocar', async () => {
  render(<App />);
  hello(withSwaps({
    pickOrderSwaps: [{ id: 8, cellId: 1, state: 'AVAILABLE' }],
    positionSwaps: [{ id: 9, cellId: 1, state: 'AVAILABLE' }],
    trades: [{ id: 7, cellId: 1, state: 'INVALID' }],
  }));
  expect(screen.queryByRole('region', { name: 'Trocas' })).not.toBeInTheDocument();
  // Um botão só no card (#641); o menu abre ao passar o mouse, com as trocas que valem agora.
  const trigger = panel().getByRole('button', { name: 'Trocar com Amigo#BR1' });
  expect(trigger).toHaveTextContent('Trocar');
  expect(trigger).toHaveAttribute('aria-expanded', 'false');
  expect(panel().queryByRole('button', { name: /Pedir troca/ })).not.toBeInTheDocument();
  fireEvent.mouseEnter(trigger);
  expect(trigger).toHaveAttribute('aria-expanded', 'true');
  const menu = () => within(screen.getByRole('menu', { name: 'Trocas com Amigo#BR1' }));
  expect(menu().getAllByRole('menuitem').map(item => item.textContent)).toEqual(['vez', 'Selva']);
  // A troca de posição mostra a rota que você recebe, a do aliado (#677).
  // Sair do botão para o menu não fecha.
  fireEvent.mouseLeave(trigger);
  fireEvent.mouseEnter(screen.getByRole('menu'));
  act(() => { vi.advanceTimersByTime(500); });
  fireEvent.click(menu().getByRole('menuitem', { name: 'Pedir troca de posição (Selva) com Amigo#BR1' }));
  expect(last().command).toEqual({ type: 'requestSwap', kind: 'position', swapId: 9 });
  expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  expect(trigger).toBeDisabled();
  await respond();
  act(() => socket().receive({
    type: 'champSelect',
    champSelect: withSwaps({ pickOrderSwaps: [{ id: 8, cellId: 1, state: 'INVALID' }], positionSwaps: [{ id: 9, cellId: 1, state: 'SENT' }] }),
  }));
  expect(panel().queryByRole('button', { name: 'Trocar com Amigo#BR1' })).not.toBeInTheDocument();
  const sent = within(panel().getByRole('group', { name: 'Seu pedido de troca de posição' }));
  expect(panel().getByRole('group', { name: 'Seu pedido de troca de posição' })).toHaveTextContent('Aguardando a resposta da troca de posição…');
  fireEvent.click(sent.getByRole('button', { name: 'Cancelar pedido' }));
  expect(last().command).toEqual({ type: 'cancelSwap', kind: 'position', swapId: 9 });
});

it('mostra "posição" quando o aliado não tem rota atribuída', () => {
  render(<App />);
  hello(withSwaps({
    myTeam: draftChampSelect.myTeam.map(p => (p.cellId === 1 ? { ...p, assignedPosition: 'unselected' } : p)),
    positionSwaps: [{ id: 9, cellId: 1, state: 'AVAILABLE' }],
  }));
  fireEvent.mouseEnter(panel().getByRole('button', { name: 'Trocar com Amigo#BR1' }));
  expect(within(screen.getByRole('menu')).getByRole('menuitem', { name: 'Pedir troca de posição com Amigo#BR1' })).toHaveTextContent('posição');
});

it('mostra só a troca que ainda vale e fecha o menu ao tirar o mouse', () => {
  render(<App />);
  // Depois do pick, a troca de posição deixa de valer e só a de campeão aparece.
  hello(withSwaps({ trades: [{ id: 7, cellId: 1, state: 'AVAILABLE' }], positionSwaps: [{ id: 9, cellId: 1, state: 'INVALID' }] }));
  const trigger = panel().getByRole('button', { name: 'Trocar com Amigo#BR1' });
  fireEvent.mouseEnter(trigger);
  expect(within(screen.getByRole('menu')).getAllByRole('menuitem').map(item => item.textContent)).toEqual(['campeão']);
  fireEvent.mouseLeave(trigger);
  act(() => { vi.advanceTimersByTime(500); });
  expect(screen.queryByRole('menu')).not.toBeInTheDocument();
});

it('abre o menu pelo teclado e fecha com Esc', () => {
  render(<App />);
  hello(withSwaps({ pickOrderSwaps: [{ id: 8, cellId: 1, state: 'AVAILABLE' }] }));
  const trigger = panel().getByRole('button', { name: 'Trocar com Amigo#BR1' });
  fireEvent.click(trigger);
  expect(screen.getByRole('menu')).toBeInTheDocument();
  fireEvent.keyDown(document, { key: 'Escape' });
  expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});

it.each([['Aceitar', 'acceptSwap'], ['Recusar', 'declineSwap']] as const)('%s um pedido recebido', async (label, type) => {
  render(<App />);
  hello(withSwaps({ trades: [{ id: 7, cellId: 1, state: 'RECEIVED' }] }));
  const request = within(panel().getByRole('group', { name: 'Pedido de troca de campeão' }));
  // O pedido fica dentro do card de quem pediu, sem aviso em cima do card do meio (#562).
  expect(within(panel().getByRole('region', { name: 'Seu time' })).getByRole('group', { name: 'Pedido de troca de campeão' })).toBeVisible();
  expect(request.getByText('Quer trocar de campeão')).toBeInTheDocument();
  expect(request.getByText('Trocar campeão?')).toBeVisible();
  fireEvent.click(request.getByRole('button', { name: label }));
  expect(last().command).toEqual({ type, kind: 'champion', swapId: 7 });
  fireEvent.click(request.getByRole('button', { name: label }));
  expect(socket().send).toHaveBeenCalledTimes(1);
  await respond({ status: 'error', code: 'invalidCommand', message: 'essa troca não está mais aberta' });
  expect(panel().getByRole('alert')).toHaveTextContent(`essa troca não está mais aberta`);
});

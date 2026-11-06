import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
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
const grid = () => within(panel().getByRole('list', { name: 'Grade de campeões' }));
const tile = (name: string) => grid().getByRole('button', { name });
const sent = (): ClientMessage[] => socket().send.mock.calls.map(([text]) => JSON.parse(text as string));
const last = () => sent().at(-1)!;
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...champSelectState, ...overrides } }));
}
const session = (change: Partial<ChampSelect>) =>
  act(() => socket().receive({ type: 'champSelect', champSelect: { ...draftChampSelect, ...change } }));
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }, id = last().id) {
  await act(async () => socket().receive({ type: 'response', id, result }));
}
const banAction = draftChampSelect.pendingAction!;
const pickAction = draftChampSelect.actions.find(a => a.id === 4)!;

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('na vez do ban, marca na grade e confirma pelo botão', async () => {
  render(<App />); hello();
  expect(panel().getByRole('button', { name: 'Banir' })).toBeDisabled();
  fireEvent.click(tile('Wukong'));
  expect(last().command).toEqual({ type: 'hoverChampion', actionId: 1, championId: 62 });
  expect(tile('Wukong')).toHaveAttribute('aria-pressed', 'true');
  await respond();
  // A sessão confirma o hover; o botão usa o campeão marcado.
  session({ pendingAction: { ...banAction, championId: 62 } });
  fireEvent.click(panel().getByRole('button', { name: 'Banir Wukong' }));
  expect(last().command).toEqual({ type: 'confirmAction', actionId: 1, championId: 62 });
  expect(panel().getByText('Confirmando o ban…')).toBeVisible();
  // Sem clique duplo nem troca de campeão enquanto confirma.
  fireEvent.click(panel().getByRole('button', { name: 'Banir Wukong' }));
  expect(tile('Aatrox')).toBeDisabled();
  expect(sent()).toHaveLength(2);
  await respond();
  session({
    pendingAction: null, bans: { myTeam: [62], theirTeam: [] },
    actions: draftChampSelect.actions.map(a => a.id === 1 ? { ...banAction, championId: 62, completed: true, isInProgress: false } : a),
  });
  expect(panel().queryByRole('button', { name: /Banir/ })).not.toBeInTheDocument();
  expect(panel().getByText(/Ainda não é sua vez/)).toBeVisible();
});

it('na vez do pick escolhe, e mostra o erro da LCU sem travar', async () => {
  render(<App />);
  hello({ champSelect: { ...draftChampSelect, pendingAction: { ...pickAction, isInProgress: true } } });
  fireEvent.click(tile('Aatrox'));
  expect(last().command).toEqual({ type: 'hoverChampion', actionId: 4, championId: 266 });
  fireEvent.click(panel().getByRole('button', { name: 'Escolher Aatrox' }));
  expect(last().command).toEqual({ type: 'confirmAction', actionId: 4, championId: 266 });
  await respond({ status: 'error', code: 'lcu', message: 'campeão indisponível' });
  expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível escolher: campeão indisponível');
  expect(panel().getByRole('button', { name: 'Escolher Aatrox' })).toBeEnabled();
});

it('fora da vez marca a intenção do pick, sem botão de confirmar', () => {
  render(<App />);
  hello({ champSelect: { ...draftChampSelect, pendingAction: null, timer: { ...draftChampSelect.timer, phase: 'PLANNING' } } });
  fireEvent.click(tile('Nunu e Willump'));
  expect(last().command).toEqual({ type: 'hoverChampion', actionId: 4, championId: 20 });
  expect(panel().queryByRole('button', { name: /Escolher/ })).not.toBeInTheDocument();
  expect(panel().getByText(/Intenção: Nunu e Willump/)).toBeVisible();
});

it('não deixa confirmar campeão que ficou indisponível', () => {
  render(<App />); hello();
  fireEvent.click(tile('Aatrox'));
  session({ pendingAction: { ...banAction, championId: 266 }, bans: { myTeam: [], theirTeam: [266] } });
  expect(panel().getByRole('button', { name: 'Banir Aatrox' })).toBeDisabled();
  expect(panel().getByText('Esse campeão não está mais disponível.')).toBeVisible();
});

it('sem ação do jogador a grade só mostra', () => {
  render(<App />);
  hello({
    champSelect: {
      ...draftChampSelect, pendingAction: null,
      actions: draftChampSelect.actions.map(a => a.actorCellId === 0 ? { ...a, completed: true, isInProgress: false } : a),
    },
  });
  expect(tile('Aatrox')).toBeDisabled();
  expect(socket().send).not.toHaveBeenCalled();
});

it('ignora a resposta atrasada de uma seleção que já acabou', async () => {
  render(<App />); hello();
  fireEvent.click(tile('Wukong'));
  fireEvent.click(panel().getByRole('button', { name: 'Banir Wukong' }));
  const id = last().id;
  session({ id: 'nova-sessao' });
  await respond({ status: 'error', code: 'lcu', message: 'antigo' }, id);
  expect(screen.queryByText(/antigo/)).not.toBeInTheDocument();
  expect(panel().getByRole('button', { name: 'Banir' })).toBeDisabled();
});

it('digitar em qualquer lugar busca, Enter marca o primeiro e Esc limpa', () => {
  render(<App />); hello();
  const search = panel().getByRole('searchbox', { name: 'Buscar campeão' });
  fireEvent.keyDown(document.body, { key: 'w' });
  expect(search).toHaveFocus();
  expect(search).toHaveValue('w');
  expect(grid().getAllByRole('button').map(b => b.title)).toEqual(['Nunu e Willump', 'Wukong']);
  fireEvent.change(search, { target: { value: 'wuk' } });
  fireEvent.keyDown(search, { key: 'Enter' });
  expect(last().command).toEqual({ type: 'hoverChampion', actionId: banAction.id, championId: 62 });
  fireEvent.keyDown(search, { key: 'Escape' });
  expect(search).toHaveValue('');
});

it('não rouba o que é digitado num campo, como o chat', () => {
  render(<App />); hello();
  const input = document.createElement('input');
  document.body.append(input);
  fireEvent.keyDown(input, { key: 'a' });
  expect(panel().getByRole('searchbox', { name: 'Buscar campeão' })).toHaveValue('');
  input.remove();
});

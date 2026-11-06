import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { champSelectState, draftChampSelect, spellCatalog } from '../test/champSelect';

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
const spells = () => within(screen.getByRole('region', { name: 'Seus feitiços' }));
const last = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({
    type: 'hello', protocol: 1, agentVersion: '0.1.0',
    state: { ...champSelectState, summonerSpellCatalog: spellCatalog, ...overrides },
  }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: last().id, result }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('mostra os feitiços atuais e troca um deles', async () => {
  render(<App />); hello();
  expect(spells().getByRole('button', { name: 'Feitiço 1 (D): Flash' })).toBeVisible();
  fireEvent.click(spells().getByRole('button', { name: 'Feitiço 2 (F): Incendiar' }));
  const list = within(spells().getByRole('list', { name: 'Escolher Feitiço 2 (F)' }));
  // Só os da fila: Marcar é do ARAM.
  expect(list.queryByRole('button', { name: 'Marcar' })).not.toBeInTheDocument();
  expect(list.getByRole('button', { name: 'Incendiar' })).toHaveAttribute('aria-pressed', 'true');
  fireEvent.click(list.getByRole('button', { name: 'Golpear' }));
  expect(last().command).toEqual({ type: 'setSummonerSpells', spell1Id: 4, spell2Id: 11 });
  expect(spells().queryByRole('list')).not.toBeInTheDocument();
  expect(spells().getByRole('button', { name: /Feitiço 1/ })).toBeDisabled();
  await respond();
  expect(spells().getByRole('button', { name: /Feitiço 1/ })).toBeEnabled();
});

it('troca os dois de lugar e não manda nada ao repetir o mesmo', () => {
  render(<App />); hello();
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 1/ }));
  fireEvent.click(spells().getByRole('button', { name: 'Flash' }));
  expect(socket().send).not.toHaveBeenCalled();
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 1/ }));
  fireEvent.click(spells().getByRole('button', { name: 'Incendiar' }));
  expect(last().command).toEqual({ type: 'setSummonerSpells', spell1Id: 14, spell2Id: 4 });
});

it('bloqueia feitiço acima do nível, mostra erro e trava quando a partida abre', async () => {
  render(<App />);
  hello({ summoner: { ...champSelectState.summoner!, summonerLevel: 8 } });
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 2/ }));
  expect(spells().getByRole('button', { name: 'Golpear' })).toBeDisabled();
  fireEvent.click(spells().getByRole('button', { name: 'Barreira' }));
  await respond({ status: 'error', code: 'invalidCommand', message: 'a seleção de campeões já acabou' });
  expect(spells().getByRole('alert')).toHaveTextContent('Não foi possível trocar os feitiços: a seleção de campeões já acabou');
  act(() => socket().receive({
    type: 'champSelect', champSelect: { ...draftChampSelect, timer: { ...draftChampSelect.timer, phase: 'GAME_STARTING' } },
  }));
  expect(spells().getByRole('button', { name: /Feitiço 1/ })).toBeDisabled();
});

it('espera o catálogo de feitiços', () => {
  render(<App />); hello({ summonerSpellCatalog: null });
  expect(spells().getByText('Carregando feitiços…')).toBeVisible();
  expect(spells().getByRole('button', { name: 'Feitiço 1 (D): Feitiço 4' })).toBeDisabled();
});


it('bloqueia também a lista já aberta quando a partida começa', () => {
  render(<App />); hello();
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 2/ }));
  act(() => socket().receive({
    type: 'champSelect', champSelect: { ...draftChampSelect, timer: { ...draftChampSelect.timer, phase: 'GAME_STARTING' } },
  }));
  const option = spells().getByRole('button', { name: 'Barreira' });
  expect(option).toBeDisabled();
  fireEvent.click(option);
  expect(socket().send).not.toHaveBeenCalled();
});

it('exibe apenas os feitiços confirmados, inclusive após timeout', async () => {
  render(<App />); hello();
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 2/ }));
  fireEvent.click(spells().getByRole('button', { name: 'Barreira' }));
  expect(spells().getByRole('button', { name: 'Feitiço 2 (F): Incendiar' })).toBeDisabled();
  await act(async () => vi.advanceTimersByTime(10000));
  expect(spells().getByRole('alert')).toHaveTextContent('Não foi possível confirmar');
  expect(spells().getByRole('button', { name: 'Feitiço 2 (F): Incendiar' })).toBeEnabled();
  await respond();
  expect(spells().getByRole('alert')).toBeVisible();
  act(() => socket().receive({ type: 'champSelect', champSelect: {
    ...draftChampSelect,
    myTeam: draftChampSelect.myTeam.map(p => p.cellId === 0 ? { ...p, spell2Id: 21 } : p),
  } }));
  expect(spells().getByRole('button', { name: 'Feitiço 2 (F): Barreira' })).toBeVisible();
  expect(socket().send).toHaveBeenCalledTimes(1);
});

it('permite Marcar no ARAM durante a finalização', () => {
  render(<App />); hello({ champSelect: {
    ...draftChampSelect, queue: 'aram', queueId: 450,
    timer: { ...draftChampSelect.timer, phase: 'FINALIZATION' },
  } });
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 2/ }));
  expect(spells().queryByRole('button', { name: 'Golpear' })).not.toBeInTheDocument();
  fireEvent.click(spells().getByRole('button', { name: 'Marcar' }));
  expect(last().command).toEqual({ type: 'setSummonerSpells', spell1Id: 4, spell2Id: 32 });
});

it.each(['seleção', 'conexão LCU'] as const)('descarta o envio anterior na nova %s', async change => {
  render(<App />); hello();
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 2/ }));
  fireEvent.click(spells().getByRole('button', { name: 'Barreira' }));
  act(() => socket().receive(change === 'seleção'
    ? { type: 'champSelect', champSelect: { ...draftChampSelect, id: 'nova-selecao' } }
    : { type: 'lcu', status: { state: 'connected', session: 2 } }));
  await respond({ status: 'error', code: 'lcu', message: 'resposta antiga' });
  expect(spells().queryByRole('alert')).not.toBeInTheDocument();
  expect(spells().getByRole('button', { name: /Feitiço 2/ })).toBeEnabled();
  expect(socket().send).toHaveBeenCalledTimes(1);
});

it.each(['agente', 'LCU'] as const)('remove o seletor ao desconectar %s e não reenvia', async source => {
  render(<App />); hello();
  fireEvent.click(spells().getByRole('button', { name: /Feitiço 2/ }));
  fireEvent.click(spells().getByRole('button', { name: 'Barreira' }));
  const oldSocket = socket();
  await act(async () => {
    if (source === 'agente') oldSocket.onclose?.();
    else oldSocket.receive({ type: 'lcu', status: { state: 'disconnected', reason: 'teste' } });
  });
  expect(screen.queryByRole('region', { name: 'Seus feitiços' })).not.toBeInTheDocument();
  if (source === 'agente') act(() => vi.advanceTimersByTime(1000));
  hello({ lcu: { state: 'connected', session: 2 } });
  if (source === 'LCU') await respond({ status: 'error', code: 'lcu', message: 'resposta antiga' });
  expect(spells().queryByRole('alert')).not.toBeInTheDocument();
  expect(spells().getByRole('button', { name: /Feitiço 2/ })).toBeEnabled();
  expect(oldSocket.send).toHaveBeenCalledTimes(1);
  if (source === 'agente') expect(socket().send).not.toHaveBeenCalled();
});

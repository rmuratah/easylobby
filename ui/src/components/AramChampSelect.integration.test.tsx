import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { aramChampSelect, champSelectState, spellCatalog } from '../test/champSelect';

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
const mine = () => within(panel().getByRole('region', { name: 'Seu campeão' }));
const bench = () => within(panel().getByRole('region', { name: 'Banco' }));
const last = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(change: Partial<ChampSelect> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0',
    state: { ...champSelectState, summonerSpellCatalog: spellCatalog, champSelect: { ...aramChampSelect, ...change } } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: last().id, result }));
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(aramChampSelect.timer.endsAtMs - 25_000);
  Socket.instances = [];
  vi.stubGlobal('WebSocket', Socket);
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('mostra o campeão sorteado, os rerolls, o banco e o time, sem grade nem runas', () => {
  render(<App />); hello();
  expect(panel().getByRole('status')).toHaveTextContent('Role de novo ou pegue um campeão do banco.');
  expect(panel().getByLabelText('Tempo da etapa')).toHaveTextContent('25 s');
  expect(mine().getByText('Aatrox')).toBeVisible();
  expect(mine().getByLabelText('Rerolls restantes')).toHaveTextContent('1 reroll restante');
  expect(mine().getByRole('button', { name: 'Rolar de novo' })).toBeEnabled();
  expect(bench().getByRole('button', { name: 'Pegar Ahri do banco' })).toBeEnabled();
  expect(bench().getByRole('button', { name: "Pegar Kai'Sa do banco" })).toHaveTextContent('prioridade');
  const team = within(panel().getByRole('region', { name: 'Seu time' }));
  expect(team.getByText('Amigo#BR1').closest('li')).toHaveTextContent('Wukong');
  expect(team.getByRole('button', { name: 'Pedir troca de campeão com Amigo#BR1' })).toBeEnabled();
  expect(panel().queryByRole('list', { name: 'Grade de campeões' })).not.toBeInTheDocument();
  expect(panel().queryByRole('region', { name: 'Time adversário' })).not.toBeInTheDocument();
  expect(panel().queryByRole('region', { name: /runas/i })).not.toBeInTheDocument();
});

it('rola de novo e acompanha o campeão novo e o banco', async () => {
  render(<App />); hello();
  fireEvent.click(mine().getByRole('button', { name: 'Rolar de novo' }));
  expect(last().command).toEqual({ type: 'rerollChampion' });
  expect(mine().getByRole('button', { name: 'Rolar de novo' })).toBeDisabled();
  expect(bench().getByRole('button', { name: 'Pegar Ahri do banco' })).toBeDisabled();
  act(() => socket().receive({ type: 'champSelect', champSelect: {
    ...aramChampSelect,
    myTeam: aramChampSelect.myTeam.map(p => p.isLocalPlayer ? { ...p, championId: 20 } : p),
    benchChampions: [...aramChampSelect.benchChampions, { championId: 266, isPriority: false }],
    rerollsRemaining: 0,
  } }));
  await respond();
  expect(mine().getByText('Nunu e Willump')).toBeVisible();
  expect(mine().queryByRole('button', { name: 'Rolar de novo' })).not.toBeInTheDocument();
  expect(bench().getByRole('button', { name: 'Pegar Aatrox do banco' })).toBeEnabled();
});

it('pega um campeão do banco e mostra o erro da LCU', async () => {
  render(<App />); hello();
  fireEvent.click(bench().getByRole('button', { name: 'Pegar Ahri do banco' }));
  expect(last().command).toEqual({ type: 'swapBenchChampion', championId: 103 });
  fireEvent.click(bench().getByRole('button', { name: "Pegar Kai'Sa do banco" }));
  expect(socket().send).toHaveBeenCalledTimes(1);
  await respond({ status: 'error', code: 'lcu', message: 'espere para trocar de novo' });
  expect(mine().getByRole('alert')).toHaveTextContent('espere para trocar de novo');
});

it('sem reroll na fila e com o banco vazio', () => {
  render(<App />); hello({ allowRerolling: false, rerollsRemaining: 0, benchChampions: [] });
  expect(mine().queryByRole('button', { name: 'Rolar de novo' })).not.toBeInTheDocument();
  expect(bench().getByText(/Nenhum campeão no banco ainda/)).toBeVisible();
});

it('a ARAM: Desordem também usa a tela do ARAM, com o nome da fila', () => {
  render(<App />);
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: {
    ...champSelectState,
    queues: [{ queueId: 2400, queue: null, name: 'ARAM: Desordem', description: '', gameMode: 'KIWI', modeGroup: 'kARAM',
      mapId: 12, versusAi: false, ranked: false, showPositionSelector: false, limitedTime: false }],
    champSelect: { ...aramChampSelect, queueId: 2400, queue: null, benchEnabled: false },
  } }));
  expect(mine().getByText('Aatrox')).toBeVisible();
  expect(panel().getByText('ARAM: Desordem')).toBeVisible();
});

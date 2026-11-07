import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { aramChampSelect, champSelectState, spellCatalog } from '../test/champSelect';
import { testRuneCatalog, testRunePages } from '../test/events';

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
    state: { ...champSelectState, runePages: testRunePages, runeCatalog: testRuneCatalog, summonerSpellCatalog: spellCatalog, champSelect: { ...aramChampSelect, ...change } } }));
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

it('mostra o campeão, os rerolls, o banco, o time e as runas, sem grade', () => {
  render(<App />); hello();
  expect(panel().getAllByRole('status')[0]).toHaveTextContent('Troque com o banco ou com um aliado.');
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
  expect(panel().getByRole('region', { name: 'Suas runas' })).toBeInTheDocument();
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
    ...champSelectState, runePages: testRunePages, runeCatalog: testRuneCatalog,
    queues: [{ queueId: 2400, queue: null, name: 'ARAM: Desordem', description: '', gameMode: 'KIWI', modeGroup: 'kARAM',
      mapId: 12, versusAi: false, ranked: false, showPositionSelector: false, limitedTime: false }],
    champSelect: { ...aramChampSelect, queueId: 2400, queue: null, benchEnabled: false },
  } }));
  expect(mine().getByText('Aatrox')).toBeVisible();
  expect(panel().getByText('ARAM: Desordem')).toBeVisible();
  // Na Desordem a troca de runas não aparece (#197).
  expect(panel().queryByRole('region', { name: 'Suas runas' })).not.toBeInTheDocument();
});

it('escolhe uma das cartas de campeão no início da seleção', async () => {
  render(<App />);
  const choosing: ChampSelect = {
    ...aramChampSelect,
    myTeam: aramChampSelect.myTeam.map(p => p.isLocalPlayer ? { ...p, championId: null } : p),
    benchChampions: [],
    allowRerolling: true, rerollsRemaining: 0,
    actions: [{ id: 0, type: 'pick', actorCellId: 0, championId: null, completed: false, isInProgress: true, isAllyAction: true, turn: 0 }],
    pendingAction: { id: 0, type: 'pick', actorCellId: 0, championId: null, completed: false, isInProgress: true, isAllyAction: true, turn: 0 },
  };
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: {
    ...champSelectState, champSelect: choosing, champSelectChampions: { pickable: null, bannable: null, subset: null } } }));
  const cards = () => within(panel().getByRole('region', { name: 'Suas cartas' }));
  expect(panel().getAllByRole('status')[0]).toHaveTextContent('Escolha uma das suas cartas.');
  expect(cards().getByText('Carregando suas cartas…')).toBeVisible();
  expect(panel().queryByRole('button', { name: 'Rolar de novo' })).not.toBeInTheDocument();

  act(() => socket().receive({ type: 'champSelectChampions', champSelectChampions: { pickable: null, bannable: null, subset: [62, 103, 266] } }));
  expect(cards().getAllByRole('button').map(b => b.getAttribute('aria-label'))).toEqual(['Escolher Wukong', 'Escolher Ahri', 'Escolher Aatrox']);
  fireEvent.click(cards().getByRole('button', { name: 'Escolher Ahri' }));
  expect(last().command).toEqual({ type: 'confirmAction', actionId: 0, championId: 103 });
  expect(cards().getByRole('button', { name: 'Escolher Aatrox' })).toBeDisabled();
  act(() => socket().receive({ type: 'champSelect', champSelect: {
    ...choosing,
    timer: { ...choosing.timer, phase: 'FINALIZATION' },
    myTeam: choosing.myTeam.map(p => p.isLocalPlayer ? { ...p, championId: 103 } : p),
    benchChampions: [{ championId: 62, isPriority: false }, { championId: 266, isPriority: false }],
    actions: [{ ...choosing.actions[0], championId: 103, completed: true, isInProgress: false }],
    pendingAction: null,
  } }));
  await respond();
  expect(panel().queryByRole('region', { name: 'Suas cartas' })).not.toBeInTheDocument();
  expect(mine().getByText('Ahri')).toBeVisible();
  expect(bench().getByRole('button', { name: 'Pegar Wukong do banco' })).toBeEnabled();
});

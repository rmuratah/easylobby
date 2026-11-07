import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
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
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...champSelectState, ...overrides } }));
}
const receive = (message: ServerMessage) => act(() => socket().receive(message));

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(draftChampSelect.timer.endsAtMs - 25_000);
  Socket.instances = [];
  vi.stubGlobal('WebSocket', Socket);
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('mostra os dois times, a etapa, a vez e o timer', () => {
  render(<App />); hello();
  expect(screen.getByRole('heading', { name: 'Seleção de campeões', level: 2 })).toBeVisible();
  expect(panel().getByText('Bans e escolhas')).toBeVisible();
  expect(panel().getByRole('status')).toHaveTextContent('Sua vez de banir.');
  expect(panel().getByLabelText('Tempo da etapa')).toHaveTextContent('25 s');
  act(() => vi.advanceTimersByTime(3000));
  expect(panel().getByLabelText('Tempo da etapa')).toHaveTextContent('22 s');
  act(() => vi.advanceTimersByTime(60_000));
  expect(panel().getByLabelText('Tempo da etapa')).toHaveTextContent('0 s');

  const mine = within(panel().getByRole('region', { name: 'Seu time' }));
  const me = mine.getByText('Victor#BR1 (você)').closest('li')!;
  expect(me).toHaveAttribute('aria-current', 'true');
  expect(me).toHaveTextContent('Sem campeão · Meio · na vez');
  expect(mine.getByText('Amigo#BR1').closest('li')).toHaveTextContent('Wukong (intenção) · Selva · na vez');
  const theirs = within(panel().getByRole('region', { name: 'Time adversário' }));
  expect(theirs.getByText('Adversário 1')).toBeVisible();
  expect(theirs.getByText('Adversário 2').closest('li')).toHaveTextContent('Ahri');
});

it('acompanha a sessão nova: etapa, vez, bans e tempo sem limite', () => {
  render(<App />); hello();
  receive({
    type: 'champSelect',
    champSelect: {
      ...draftChampSelect, pendingAction: null, bans: { myTeam: [266], theirTeam: [145] },
      timer: { ...draftChampSelect.timer, phase: 'FINALIZATION', isInfinite: true },
    },
  });
  expect(panel().getByText('Finalização')).toBeVisible();
  expect(panel().getByRole('status')).toHaveTextContent('Ajuste feitiços e runas');
  expect(panel().getByLabelText('Tempo da etapa')).toHaveTextContent('sem limite');
  expect(within(panel().getByRole('region', { name: 'Bans do seu time' })).getByText('Aatrox')).toBeInTheDocument();
  expect(within(panel().getByRole('region', { name: "Bans do time adversário" })).getByText("Kai'Sa")).toBeInTheDocument();
  // Fora do ban a grade mostra os que dá para escolher, e o banido fica indisponível.
  expect(panel().getByRole('region', { name: 'Campeões' })).toBeVisible();
  expect(grid().getByRole('button', { name: 'Aatrox' })).toBeDisabled();
  expect(grid().queryByRole('button', { name: "Kai'Sa" })).not.toBeInTheDocument();
});

it('busca na grade e marca a escolha', () => {
  render(<App />); hello();
  expect(panel().getByRole('region', { name: 'Campeões para banir' })).toBeVisible();
  expect(grid().getAllByRole('button')).toHaveLength(5);
  expect(grid().getByRole('button', { name: 'Ahri' })).toBeDisabled();
  fireEvent.change(panel().getByRole('searchbox', { name: 'Buscar campeão' }), { target: { value: 'monkey' } });
  expect(grid().getAllByRole('button')).toHaveLength(1);
  fireEvent.click(grid().getByRole('button', { name: 'Wukong' }));
  expect(grid().getByRole('button', { name: 'Wukong' })).toHaveAttribute('aria-pressed', 'true');
  fireEvent.change(panel().getByRole('searchbox', { name: 'Buscar campeão' }), { target: { value: 'xyz' } });
  expect(panel().getByText('Nenhum campeão encontrado.')).toBeVisible();
});

it('preserva a busca na navegação e recomeça numa seleção nova', () => {
  render(<App />); hello();
  const search = () => panel().getByRole('searchbox', { name: 'Buscar campeão' });
  fireEvent.change(search(), { target: { value: 'ah' } });
  fireEvent.click(screen.getByRole('button', { name: 'Início' }));
  expect(screen.queryByRole('region', { name: 'Seleção de campeões' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Lobby' }));
  expect(search()).toHaveValue('ah');
  receive({ type: 'champSelect', champSelect: { ...draftChampSelect, id: 'outra-sessao' } });
  expect(search()).toHaveValue('');
});

it('espera a sessão e o catálogo, e some quando a seleção fecha', () => {
  render(<App />); hello({ champSelect: null, championCatalog: null });
  expect(screen.getByText('Carregando a seleção de campeões…')).toBeVisible();
  receive({ type: 'champSelect', champSelect: draftChampSelect });
  expect(panel().getByText('Carregando campeões…')).toBeVisible();
  receive({ type: 'championCatalog', championCatalog: champSelectState.championCatalog });
  expect(grid().getAllByRole('button')).toHaveLength(5);
  receive({ type: 'champSelect', champSelect: null });
  receive({ type: 'gameflowPhase', phase: 'GameStart' });
  expect(screen.queryByRole('region', { name: 'Seleção de campeões' })).not.toBeInTheDocument();
  expect(screen.getByText(/A partida está abrindo/)).toBeVisible();
});

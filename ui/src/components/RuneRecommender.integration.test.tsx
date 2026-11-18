import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import { recommendationTitle, recommenderChampion, recommenderLane } from '../agent/runeRecommender';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { RecommendedRunes } from '../bindings/RecommendedRunes';
import type { ServerMessage } from '../bindings/ServerMessage';
import { champSelectState, draftChampSelect, spellCatalog } from '../test/champSelect';
import { testChampionCatalog } from '../test/events';
import { testFullRuneCatalog, testRunePage } from '../test/runes';

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
const last = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const runes = () => within(screen.getByRole('region', { name: 'Suas runas' }));

/** Seleção com o jogador logado (meio) marcando a Ahri. */
function withChampion(change: Partial<ChampSelect['myTeam'][number]> = {}): ChampSelect {
  return {
    ...draftChampSelect,
    myTeam: draftChampSelect.myTeam.map(p => p.cellId === 0 ? { ...p, championPickIntent: 103, ...change } : p),
  };
}

function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({
    type: 'hello', protocol: 1, agentVersion: '0.1.0',
    state: {
      ...champSelectState, champSelect: withChampion(), summonerSpellCatalog: spellCatalog, runeCatalog: testFullRuneCatalog,
      runePages: { pages: [testRunePage], currentPageId: 10, canCreatePage: true }, ...overrides,
    },
  }));
}

const suggestions = (position: RecommendedRunes['position'] = 'middle'): RecommendedRunes => ({
  championId: 103, position,
  pages: [
    { title: 'Executar inimigos e Ouro', primaryStyleId: 8100, subStyleId: 8000, selectedPerkIds: [202, 211, 221, 231, 111, 121, 5008, 5008, 5011], summonerSpellIds: [4, 14], recommendationId: 'a' },
    { title: '', primaryStyleId: 8200, subStyleId: 8000, selectedPerkIds: [302, 311, 321, 331, 111, 121, 5008, 5008, 5011], summonerSpellIds: [4, 14], recommendationId: 'b' },
  ],
});

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('só mostra o recomendador com um campeão escolhido ou marcado', () => {
  render(<App />); hello({ champSelect: draftChampSelect });
  expect(runes().queryByRole('button', { name: 'Recomendador de runas' })).toBeNull();
  hello();
  expect(runes().getByRole('button', { name: 'Recomendador de runas' })).toBeVisible();
});

it('mostra as sugestões da rota e aplica a escolhida', async () => {
  render(<App />); hello();
  fireEvent.click(runes().getByRole('button', { name: 'Recomendador de runas' }));
  const dialog = screen.getByRole('dialog', { name: /Recomendador/ });
  expect(dialog).toHaveTextContent('Ahri');
  expect(last()).toMatchObject({ command: { type: 'openRecommendedRunes', championId: 103, position: 'middle' } });
  expect(within(dialog).getByText('Carregando as sugestões…')).toBeInTheDocument();

  act(() => socket().receive({ type: 'recommendedRunes', recommendedRunes: suggestions() }));
  await act(async () => socket().receive({ type: 'response', id: last().id, result: { status: 'ok' } }));
  const pages = within(dialog).getByRole('list', { name: 'Páginas sugeridas' });
  expect(within(pages).getAllByRole('listitem')).toHaveLength(2);
  expect(within(pages).getByText('Executar inimigos e Ouro')).toBeInTheDocument();
  // Sem o nome da LCU, a runa principal e a árvore secundária.
  expect(within(pages).getByText('Cometa Arcano · Precisão')).toBeInTheDocument();

  fireEvent.click(within(pages).getByRole('button', { name: 'Usar a página Cometa Arcano · Precisão' }));
  expect(last()).toMatchObject({ command: { type: 'applyRecommendedRunes', index: 1 } });
  expect(within(dialog).getByRole('status')).toHaveTextContent('Aplicando a página…');
  await act(async () => socket().receive({ type: 'response', id: last().id, result: { status: 'ok' } }));
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('troca a rota e pede as sugestões dela', () => {
  render(<App />); hello({ recommendedRunes: suggestions() });
  fireEvent.click(runes().getByRole('button', { name: 'Recomendador de runas' }));
  const dialog = screen.getByRole('dialog', { name: /Recomendador/ });
  expect(within(dialog).getByText('Executar inimigos e Ouro')).toBeInTheDocument();
  fireEvent.click(within(dialog).getByRole('button', { name: 'Topo' }));
  expect(last()).toMatchObject({ command: { type: 'openRecommendedRunes', championId: 103, position: 'top' } });
  // As sugestões do meio não valem para o topo.
  expect(within(dialog).queryByText('Executar inimigos e Ouro')).toBeNull();
  act(() => socket().receive({ type: 'recommendedRunes', recommendedRunes: { ...suggestions('top'), pages: [] } }));
  expect(within(dialog).getByText('O client não tem sugestões para Ahri como Topo.')).toBeInTheDocument();
});

it('abre o editor depois de aplicar, se pedido', async () => {
  render(<App />); hello({ recommendedRunes: suggestions() });
  fireEvent.click(runes().getByRole('button', { name: 'Recomendador de runas' }));
  const dialog = screen.getByRole('dialog', { name: /Recomendador/ });
  fireEvent.click(within(dialog).getByRole('checkbox', { name: 'Editar a página depois de aplicar' }));
  fireEvent.click(within(dialog).getByRole('button', { name: 'Usar a página Executar inimigos e Ouro' }));
  await act(async () => socket().receive({ type: 'response', id: last().id, result: { status: 'ok' } }));
  expect(screen.getByRole('dialog', { name: 'Editar runas' })).toBeVisible();
});

it('mostra o erro e continua aberto quando a LCU recusa', async () => {
  render(<App />); hello({ recommendedRunes: suggestions() });
  fireEvent.click(runes().getByRole('button', { name: 'Recomendador de runas' }));
  const dialog = screen.getByRole('dialog', { name: /Recomendador/ });
  fireEvent.click(within(dialog).getByRole('button', { name: 'Usar a página Executar inimigos e Ouro' }));
  await act(async () => socket().receive({ type: 'response', id: last().id, result: { status: 'error', code: 'lcu', message: 'recusado' } }));
  expect(within(dialog).getByRole('alert')).toHaveTextContent('Não foi possível aplicar a página: recusado');
});

describe('runeRecommender', () => {
  it('usa o campeão escolhido antes da intenção', () => {
    expect(recommenderChampion(draftChampSelect)).toBeNull();
    expect(recommenderChampion(withChampion())).toBe(103);
    expect(recommenderChampion(withChampion({ championId: 266 }))).toBe(266);
  });

  it('começa na rota do jogador ou na do campeão', () => {
    expect(recommenderLane(withChampion(), testChampionCatalog, 103)).toBe('middle');
    expect(recommenderLane(withChampion({ assignedPosition: 'unselected' }), testChampionCatalog, 62)).toBe('jungle');
    expect(recommenderLane(withChampion({ assignedPosition: 'fill' }), null, 62)).toBe('middle');
  });

  it('nome da sugestão sem nada', () => {
    expect(recommendationTitle({ ...suggestions().pages[1], selectedPerkIds: [], subStyleId: 1 }, null)).toBe('Página sugerida');
  });
});

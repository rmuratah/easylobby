import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { RunePage } from '../bindings/RunePage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { champSelectState, spellCatalog } from '../test/champSelect';
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
const top: RunePage = { ...testRunePage, id: 11, name: 'Top' };
const runes = () => within(screen.getByRole('region', { name: 'Suas runas' }));
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({
    type: 'hello', protocol: 1, agentVersion: '0.1.0',
    state: {
      ...champSelectState, summonerSpellCatalog: spellCatalog, runeCatalog: testFullRuneCatalog,
      runePages: { pages: [testRunePage, top], currentPageId: 10, canCreatePage: true }, ...overrides,
    },
  }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('troca a página de runas sem sair da seleção', async () => {
  render(<App />); hello();
  const select = runes().getByRole('combobox', { name: 'Página de runas' });
  expect(select).toHaveValue('10');
  expect(runes().getByText('Pressione o Ataque · Dominação')).toBeInTheDocument();
  fireEvent.change(select, { target: { value: '11' } });
  expect(last()).toMatchObject({ command: { type: 'selectRunePage', pageId: 11 } });
  expect(runes().getByRole('status')).toHaveTextContent('Aplicando página…');
  expect(select).toBeDisabled();
  await act(async () => socket().receive({ type: 'response', id: last().id, result: { status: 'ok' } }));
  // A página só muda quando o agente confirma pelo estado.
  expect(select).toHaveValue('10');
  expect(select).toBeEnabled();
});

it('abre o editor de runas num pop-up por cima da seleção e devolve o foco (#429)', () => {
  render(<App />); hello();
  const edit = runes().getByRole('button', { name: 'Editar runas' });
  edit.focus();
  fireEvent.click(edit);
  const dialog = screen.getByRole('dialog', { name: 'Editar runas' });
  expect(within(dialog).getByRole('region', { name: 'Páginas de runas' })).toBeVisible();
  // A seleção continua na tela, por baixo.
  expect(screen.getByRole('region', { name: 'Seleção de campeões', hidden: true })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /Voltar para/ })).toBeNull();
  expect(within(dialog).getByRole('combobox', { name: 'Página de runas' })).toHaveFocus();
  fireEvent.click(within(dialog).getByRole('button', { name: 'Fechar' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(edit).toHaveFocus();
  // Esc também fecha.
  fireEvent.click(edit);
  fireEvent.keyDown(document, { key: 'Escape' });
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('espera as páginas chegarem', () => {
  render(<App />); hello({ runePages: null });
  expect(runes().getByText('Carregando páginas de runas…')).toBeInTheDocument();
});

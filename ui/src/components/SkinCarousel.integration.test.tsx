import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampionSkins } from '../bindings/ChampionSkins';
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

// O jogador (célula 0) já confirmou a Ahri (103).
const locked: ChampSelect = {
  ...draftChampSelect,
  pendingAction: null,
  myTeam: draftChampSelect.myTeam.map(p => p.isLocalPlayer ? { ...p, championId: 103, selectedSkinId: 103000 } : p),
  actions: draftChampSelect.actions.map(a => a.id === 4 ? { ...a, championId: 103, completed: true } : a),
};
const skins: ChampionSkins = {
  championId: 103,
  skins: [
    { id: 103000, name: 'Ahri', isBase: true, owned: true, rarity: null, chromas: [] },
    { id: 103001, name: 'Ahri Fliperama', isBase: false, owned: true, rarity: 'epic', chromas: [] },
    { id: 103002, name: 'Ahri Projeto', isBase: false, owned: false, rarity: 'legendary', chromas: [] },
  ],
};

beforeEach(() => { Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.unstubAllGlobals(); });

it('antes de confirmar o campeão mostra a grade, sem o carrossel', () => {
  render(<App />);
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: champSelectState }));
  expect(panel().getByRole('list', { name: 'Grade de campeões' })).toBeInTheDocument();
  expect(panel().queryByRole('region', { name: 'Skins' })).not.toBeInTheDocument();
});

it('com o campeão confirmado, a grade vira o carrossel e clicar numa skin troca na LCU', () => {
  render(<App />);
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...champSelectState, champSelect: locked } }));
  expect(panel().queryByRole('list', { name: 'Grade de campeões' })).not.toBeInTheDocument();
  const carousel = within(panel().getByRole('region', { name: 'Skins' }));
  expect(last().command).toEqual({ type: 'openChampionSkins', championId: 103 });
  expect(carousel.getByText('Carregando as skins…')).toBeInTheDocument();

  act(() => socket().receive({ type: 'championSkins', championSkins: skins }));
  const thumbs = within(carousel.getByRole('list', { name: 'Miniaturas das skins' }));
  expect(thumbs.getByRole('button', { name: 'Ahri (padrão)' })).toHaveAttribute('aria-pressed', 'true');
  // A que a conta não tem só mostra a splash: não manda nada à LCU (#494).
  const notOwned = thumbs.getByRole('button', { name: 'Ahri Projeto (você não tem; só ver)' });
  const before = sent().length;
  fireEvent.click(notOwned);
  expect(sent()).toHaveLength(before);
  expect(carousel.getByRole('status')).toHaveTextContent('Ahri Projeto');
  expect(carousel.getByRole('status')).toHaveTextContent('Você não tem esta skin');
  expect(notOwned).toHaveAttribute('aria-current', 'true');
  expect(notOwned).toHaveAttribute('aria-pressed', 'false');
  expect(thumbs.getByRole('button', { name: 'Ahri (padrão)' })).toHaveAttribute('aria-pressed', 'true');
  // Clicar de novo fecha a visualização.
  fireEvent.click(notOwned);
  expect(carousel.getByRole('status')).toHaveTextContent('Ahri');
  expect(carousel.getByRole('status')).not.toHaveTextContent('Você não tem');
  fireEvent.click(notOwned);

  fireEvent.click(thumbs.getByRole('button', { name: 'Ahri Fliperama' }));
  expect(last().command).toEqual({ type: 'selectSkin', skinId: 103001 });
  expect(thumbs.getByRole('button', { name: 'Ahri Fliperama' })).toHaveAttribute('aria-pressed', 'true');
  expect(carousel.getByRole('status')).toHaveTextContent('Ahri Fliperama');
});

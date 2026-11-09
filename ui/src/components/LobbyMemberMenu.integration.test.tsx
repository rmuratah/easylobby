import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { Lobby } from '../bindings/Lobby';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testFriend } from '../test/events';
import { testLobby, testState } from '../test/lobby';

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
const panel = () => within(screen.getByRole('region', { name: 'Lobby' }));
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const friend = { ...testLobby.members[0], puuid: 'ana', summonerId: 2, isLeader: false };
const party: Lobby = { ...testLobby, maxLobbySize: 5, queueId: 400, queue: 'normalDraft', members: [...testLobby.members, friend] };
const friends: AgentState['friends'] = [{ ...testFriend, puuid: 'ana', gameName: 'Ana', tagLine: 'BR1', summonerId: 2 }];
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, gameflowPhase: 'Lobby', friends, ...overrides } }));
}
const menuButton = () => panel().getByRole('button', { name: /^Ações para Ana/ });
beforeEach(() => { configure({ defaultHidden: true }); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

describe('remover membro e passar a liderança (#267)', () => {
  it('o líder vê o menu só no card dos outros', () => {
    render(<App />);
    hello({ lobby: party });
    expect(menuButton()).toBeEnabled();
    expect(panel().getAllByRole('button', { name: /^Ações para/ })).toHaveLength(1);
  });

  it('quem não é o líder não vê o menu', () => {
    render(<App />);
    hello({ lobby: { ...party, isLeader: false, members: [{ ...party.members[0], isLeader: false }, { ...friend, isLeader: true }] } });
    expect(panel().queryByRole('button', { name: /^Ações para/ })).not.toBeInTheDocument();
  });

  it('passa a liderança depois de confirmar, e o menu some com o lobby novo', async () => {
    render(<App />);
    hello({ lobby: party });
    fireEvent.click(menuButton());
    fireEvent.click(panel().getByRole('menuitem', { name: 'Tornar líder' }));
    const dialog = panel().getByRole('alertdialog', { name: 'Tornar líder' });
    expect(dialog).toHaveTextContent('Passar a liderança para Ana');
    const sent = socket().send.mock.calls.length;
    fireEvent.click(within(dialog).getByRole('button', { name: 'Tornar líder' }));
    expect(socket().send.mock.calls.length).toBe(sent + 1);
    expect(request().command).toEqual({ type: 'promoteLobbyMember', summonerId: 2 });
    const id = request().id;
    await act(async () => {
      socket().receive({ type: 'lobby', lobby: { ...party, isLeader: false,
        members: [{ ...party.members[0], isLeader: false }, { ...friend, isLeader: true }] } });
      socket().receive({ type: 'response', id, result: { status: 'ok' } });
    });
    expect(panel().getByText('Você é membro')).toBeInTheDocument();
    expect(panel().queryByRole('button', { name: /^Ações para/ })).not.toBeInTheDocument();
  });

  it('cancelar não manda nada', () => {
    render(<App />);
    hello({ lobby: party });
    const sent = socket().send.mock.calls.length;
    fireEvent.click(menuButton());
    fireEvent.click(panel().getByRole('menuitem', { name: 'Remover do grupo' }));
    fireEvent.click(panel().getByRole('button', { name: 'Cancelar' }));
    expect(socket().send.mock.calls.length).toBe(sent);
    expect(panel().getByRole('menuitem', { name: 'Remover do grupo' })).toBeInTheDocument();
  });

  it('remove do grupo e mostra o erro quando a LCU recusa', async () => {
    render(<App />);
    hello({ lobby: party });
    fireEvent.click(menuButton());
    fireEvent.click(panel().getByRole('menuitem', { name: 'Remover do grupo' }));
    const dialog = panel().getByRole('alertdialog', { name: 'Remover do grupo' });
    expect(dialog).toHaveTextContent('Remover Ana#BR1 do grupo?');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Remover' }));
    expect(request().command).toEqual({ type: 'kickLobbyMember', summonerId: 2 });
    expect(menuButton()).toBeDisabled();
    const id = request().id;
    await act(async () => socket().receive({ type: 'response', id, result: { status: 'error', code: 'lcu', message: 'recusado' } }));
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível remover do grupo: recusado');
    expect(menuButton()).toBeEnabled();
  });

  it('fica desabilitado na fila', () => {
    render(<App />);
    hello({ lobby: party, gameflowPhase: 'Matchmaking' });
    expect(menuButton()).toBeDisabled();
  });
});

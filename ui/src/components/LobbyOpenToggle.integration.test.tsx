import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
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
const toggle = () => panel().getByRole('switch', { name: /Lobby (aberto|fechado)/ });
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, gameflowPhase: 'Lobby', ...overrides } }));
}
beforeEach(() => { configure({ defaultHidden: true }); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

describe('lobby aberto ou fechado (#225)', () => {
  it('o líder abre e fecha, e a tela mostra o que o agente confirmou', async () => {
    render(<App />);
    hello({ lobby: testLobby });
    expect(toggle()).toHaveAccessibleName('Lobby fechado');
    expect(toggle()).toHaveAttribute('aria-checked', 'false');
    fireEvent.click(toggle());
    expect(request().command).toEqual({ type: 'setLobbyOpen', open: true });
    const id = request().id;
    await act(async () => {
      socket().receive({ type: 'lobby', lobby: { ...testLobby, isOpen: true } });
      socket().receive({ type: 'response', id, result: { status: 'ok' } });
    });
    expect(toggle()).toHaveAttribute('aria-checked', 'true');
    expect(toggle()).toHaveAccessibleName('Lobby aberto');
    fireEvent.click(toggle());
    expect(request().command).toEqual({ type: 'setLobbyOpen', open: false });
  });

  it('mostra o erro quando a LCU recusa', async () => {
    render(<App />);
    hello({ lobby: testLobby });
    fireEvent.click(toggle());
    const id = request().id;
    await act(async () => socket().receive({ type: 'response', id, result: { status: 'error', code: 'lcu', message: 'recusado' } }));
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível abrir o lobby: recusado');
    expect(toggle()).toHaveAttribute('aria-checked', 'false');
  });

  it('fica desabilitado para quem não é o líder', () => {
    render(<App />);
    const member = { ...testLobby.members[0], isLeader: false };
    hello({ lobby: { ...testLobby, isLeader: false, isOpen: true, members: [member] } });
    expect(toggle()).toBeDisabled();
    expect(toggle()).toHaveAttribute('aria-checked', 'true');
  });

  it('não aparece no Tutorial', () => {
    render(<App />);
    hello({ lobby: { ...testLobby, queueId: 2010, queue: null, showPositionSelector: false } });
    expect(panel().queryByRole('switch')).not.toBeInTheDocument();
  });
});

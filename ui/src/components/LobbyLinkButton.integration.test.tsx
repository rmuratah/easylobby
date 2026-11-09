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
const button = () => panel().getByRole('button', { name: /Copiar link|Link copiado/ });
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const link = { url: 'https://gg.riotgames.com/LOL?joinCode=ABCD-EFGH-IJKL', joinCode: 'ABCD-EFGH-IJKL', expiresAt: null };
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, gameflowPhase: 'Lobby', ...overrides } }));
}
let writeText: ReturnType<typeof vi.fn>;
beforeEach(() => {
  configure({ defaultHidden: true });
  Socket.instances = [];
  vi.stubGlobal('WebSocket', Socket);
  writeText = vi.fn(() => Promise.resolve());
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
});
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

describe('link para entrar no lobby (#226)', () => {
  it('pede o link ao agente e copia para a área de transferência', async () => {
    render(<App />);
    hello({ lobby: testLobby });
    fireEvent.click(button());
    expect(request().command).toEqual({ type: 'createLobbyLink' });
    const id = request().id;
    await act(async () => {
      socket().receive({ type: 'lobbyLink', lobbyLink: link });
      socket().receive({ type: 'response', id, result: { status: 'ok' } });
    });
    expect(writeText).toHaveBeenCalledWith(link.url);
    expect(button()).toHaveAccessibleName('Link copiado');
  });

  it('mostra o link na tela quando não dá para copiar', async () => {
    writeText.mockImplementation(() => Promise.reject(new Error('negado')));
    render(<App />);
    hello({ lobby: testLobby });
    fireEvent.click(button());
    const id = request().id;
    await act(async () => {
      socket().receive({ type: 'lobbyLink', lobbyLink: link });
      socket().receive({ type: 'response', id, result: { status: 'ok' } });
    });
    expect(panel().getByText(link.url)).toBeInTheDocument();
    expect(button()).toHaveAccessibleName('Copiar link');
  });

  it('mostra o erro quando a LCU recusa', async () => {
    render(<App />);
    hello({ lobby: testLobby });
    fireEvent.click(button());
    const id = request().id;
    await act(async () => socket().receive({ type: 'response', id, result: { status: 'error', code: 'lcu', message: 'recusado' } }));
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível gerar o link: recusado');
    expect(writeText).not.toHaveBeenCalled();
  });

  it('copia o link também durante a busca', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking' });
    expect(button()).toBeEnabled();
    fireEvent.click(button());
    expect(request().command).toEqual({ type: 'createLobbyLink' });
    const id = request().id;
    await act(async () => {
      socket().receive({ type: 'lobbyLink', lobbyLink: link });
      socket().receive({ type: 'response', id, result: { status: 'ok' } });
    });
    expect(writeText).toHaveBeenCalledWith(link.url);
  });

  it.each(['ReadyCheck', 'ChampSelect', 'InProgress'] as const)('fica desabilitado na fase %s', phase => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: phase });
    expect(button()).toBeDisabled();
  });

  it('não aparece no Tutorial', () => {
    render(<App />);
    hello({ lobby: { ...testLobby, queueId: 2000 } });
    expect(panel().queryByRole('button', { name: /Copiar link/ })).toBeNull();
  });
});

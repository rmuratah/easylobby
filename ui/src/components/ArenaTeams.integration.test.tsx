import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { Lobby } from '../bindings/Lobby';
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
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, gameflowPhase: 'Lobby', ...overrides } }));
}
const arenaLobby: Lobby = {
  ...testLobby, queueId: 1750, queue: null, maxLobbySize: 18, showPositionSelector: false, subteams: { count: 6, size: 3 },
  members: [
    { ...testLobby.members[0], subteam: { team: 1, position: 1 } },
    { ...testLobby.members[0], puuid: 'amigo', summonerId: 2, isLeader: false, subteam: { team: 2, position: 3 } },
  ],
};
beforeEach(() => { configure({ defaultHidden: true }); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

describe('lobby da Arena com equipes (#224)', () => {
  it('mostra 6 equipes de 3, com o jogador na sua vaga e a própria equipe marcada', () => {
    render(<App />);
    hello({ lobby: arenaLobby });
    const teams = panel().getByRole('list', { name: 'Equipes do lobby' });
    expect(within(teams).getAllByRole('listitem', { name: /^Equipe \d$/ })).toHaveLength(6);
    const first = within(teams).getByRole('listitem', { name: 'Equipe 1' });
    expect(first).toHaveTextContent('Sua equipe');
    expect(within(first).getByRole('listitem', { name: 'Victor#BR1, vaga 1 da Equipe 1' })).toBeInTheDocument();
    expect(within(first).getAllByRole('listitem', { name: /^Vaga livre/ })).toHaveLength(2);
    expect(within(teams).getByRole('listitem', { name: 'Equipe 2' })).not.toHaveTextContent('Sua equipe');
    // 18 vagas, 2 ocupadas.
    expect(panel().getAllByRole('button', { name: /^Trocar para a vaga/ })).toHaveLength(16);
    expect(panel().queryByRole('list', { name: 'Jogadores no lobby' })).not.toBeInTheDocument();
  });

  it('troca de vaga pelo agente e mostra o erro da LCU', async () => {
    render(<App />);
    hello({ lobby: arenaLobby });
    fireEvent.click(panel().getByRole('button', { name: 'Trocar para a vaga 2 da Equipe 4' }));
    expect(request().command).toEqual({ type: 'setSubteamSlot', team: 4, position: 2 });
    expect(panel().getByRole('button', { name: 'Trocar para a vaga 1 da Equipe 3' })).toBeDisabled();
    const id = request().id;
    await act(async () => socket().receive({ type: 'response', id, result: { status: 'error', code: 'lcu', message: 'vaga ocupada' } }));
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível trocar de vaga: vaga ocupada');
    expect(panel().getByRole('button', { name: 'Trocar para a vaga 1 da Equipe 3' })).toBeEnabled();
  });

  it('convida pela vaga livre e não troca de vaga na fila', () => {
    render(<App />);
    hello({ lobby: arenaLobby, gameflowPhase: 'Matchmaking' });
    expect(panel().getByRole('button', { name: 'Trocar para a vaga 2 da Equipe 1' })).toBeDisabled();
    const invite = panel().getByRole('button', { name: 'Convidar amigo para a vaga 2 da Equipe 1' });
    fireEvent.click(invite);
    expect(invite).toHaveAttribute('aria-expanded', 'true');
  });

  it('sem permissão de convidar, a vaga livre só tem a troca', () => {
    render(<App />);
    hello({ lobby: { ...arenaLobby, canInvite: false } });
    expect(panel().queryByRole('button', { name: /^Convidar amigo para/ })).not.toBeInTheDocument();
  });
});

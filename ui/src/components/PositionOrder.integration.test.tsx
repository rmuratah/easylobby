import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { Lobby } from '../bindings/Lobby';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testLobby, testState } from '../test/lobby';
import { moveLane } from '../agent/positions';

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
const dialog = () => within(screen.getByRole('dialog', { name: 'Defina sua preferência de posição' }));
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
beforeEach(() => { configure({ defaultHidden: true }); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

/** Lobby do Rift Clássico (Jade, 4310) com as 5 posições em ordem. */
const jadeLobby: Lobby = { ...testLobby, queueId: 4310, queue: null, orderedPositions: true, maxLobbySize: 5 };
const order = () => dialog().getAllByRole('listitem').map(item => item.textContent?.replace(/^\d|[▲▼]/g, '').trim());

describe('preferências de posição do Rift Clássico (#230)', () => {
  it('move uma rota e empurra as do meio', () => {
    expect(moveLane(['top', 'jungle', 'middle', 'bottom', 'utility'], 3, 0)).toEqual(['bottom', 'top', 'jungle', 'middle', 'utility']);
    expect(moveLane(['top', 'jungle', 'middle', 'bottom', 'utility'], 0, 9)).toEqual(['top', 'jungle', 'middle', 'bottom', 'utility']);
  });

  it('ordena pelas setas e salva a ordem só no Salvar', async () => {
    render(<App />);
    hello({ gameflowPhase: 'Lobby', lobby: jadeLobby });
    expect(panel().queryByRole('combobox', { name: 'Posição primária' })).not.toBeInTheDocument();
    expect(panel().getByText(/Ordene as 5 posições/)).toBeInTheDocument();
    fireEvent.click(panel().getByRole('button', { name: 'Definir preferências' }));
    expect(order()).toEqual(['1ª: Topo', '2ª: Selva', '3ª: Meio', '4ª: Atirador', '5ª: Suporte']);
    fireEvent.click(dialog().getByRole('button', { name: 'Subir Meio' }));
    fireEvent.click(dialog().getByRole('button', { name: 'Subir Meio' }));
    fireEvent.click(dialog().getByRole('button', { name: 'Descer Topo' }));
    expect(order()).toEqual(['1ª: Meio', '2ª: Selva', '3ª: Topo', '4ª: Atirador', '5ª: Suporte']);
    expect(socket().send).not.toHaveBeenCalled();
    fireEvent.click(dialog().getByRole('button', { name: 'Salvar' }));
    expect(request().command).toEqual({ type: 'setPositionOrder', positions: ['middle', 'jungle', 'top', 'bottom', 'utility'] });
    const saved = { ...jadeLobby, members: [{ ...jadeLobby.members[0], firstPosition: 'middle' as const,
      positionOrder: ['middle', 'jungle', 'top', 'bottom', 'utility'] as Lobby['members'][0]['positionOrder'] }] };
    await act(async () => {
      socket().receive({ type: 'lobby', lobby: saved });
      socket().receive({ type: 'response', id: request().id, result: { status: 'ok' } });
    });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    const mine = panel().getByRole('group', { name: 'Preferências de posição de Victor#BR1' });
    expect(mine).toHaveTextContent('Meio');
    expect(within(mine).getByRole('list', { name: 'Da 2ª à 5ª preferência' })).toHaveTextContent('2ª: Selva');
    expect(panel().getByRole('button', { name: 'Mudar preferências' })).toBeEnabled();
  });

  it('salva Qualquer posição e mostra o erro da LCU sem fechar o pop-up', async () => {
    render(<App />);
    hello({ gameflowPhase: 'Lobby', lobby: jadeLobby });
    fireEvent.click(panel().getByRole('button', { name: 'Definir preferências' }));
    fireEvent.click(dialog().getByRole('checkbox', { name: 'Qualquer posição' }));
    expect(dialog().getByRole('button', { name: 'Subir Selva' })).toBeDisabled();
    fireEvent.click(dialog().getByRole('button', { name: 'Salvar' }));
    expect(request().command).toEqual({ type: 'setPositionOrder', positions: ['fill'] });
    await act(async () => socket().receive({ type: 'response', id: request().id,
      result: { status: 'error', code: 'lcu', message: 'recusado' } }));
    expect(dialog().getByRole('alert')).toHaveTextContent('recusado');
    fireEvent.click(dialog().getByRole('button', { name: 'Cancelar' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('não deixa mudar fora da fase de lobby', () => {
    render(<App />);
    hello({ gameflowPhase: 'Matchmaking', lobby: { ...jadeLobby, members: [{ ...jadeLobby.members[0], positionOrder: ['fill'] }] } });
    expect(panel().getByRole('button', { name: 'Mudar preferências' })).toBeDisabled();
    expect(panel().getByRole('group', { name: /Preferências de posição de/ })).toHaveTextContent('Qualquer posição');
  });
});

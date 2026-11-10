import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
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
const panel = () => within(screen.getByRole('region', { name: 'Fim de partida' }));
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: request().id, result }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('volta ao lobby pelo playAgain e troca para o lobby quando a fase muda', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame' });
  fireEvent.click(panel().getByRole('button', { name: 'Voltar ao lobby' }));
  expect(request().command).toEqual({ type: 'playAgain' });
  expect(panel().getByRole('button', { name: 'Voltando ao lobby…' })).toBeDisabled();
  await respond();
  act(() => socket().receive({ type: 'lobby', lobby: testLobby }));
  act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
  expect(screen.queryByRole('region', { name: 'Fim de partida' })).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Sair do lobby' })).toBeEnabled();
});

it('mostra o erro da LCU e deixa tentar de novo', async () => {
  render(<App />);
  hello({ gameflowPhase: 'PreEndOfGame' });
  fireEvent.click(panel().getByRole('button', { name: 'Voltar ao lobby' }));
  await respond({ status: 'error', code: 'lcu', message: 'fora do pós-jogo' });
  expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível voltar ao lobby: fora do pós-jogo');
  expect(panel().getByRole('button', { name: 'Voltar ao lobby' })).toBeEnabled();
});

it('avisa que o resultado está sendo calculado em WaitingForStats', () => {
  render(<App />);
  hello({ gameflowPhase: 'WaitingForStats' });
  expect(panel().getByText('Calculando o resultado da partida.')).toBeInTheDocument();
  expect(panel().getByRole('button', { name: 'Voltar ao lobby' })).toBeEnabled();
});

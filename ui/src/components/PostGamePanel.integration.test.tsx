import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import { HONOR_CONFIRM_MS } from './HonorVote';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { HonorBallot } from '../bindings/HonorBallot';
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

const ballot: HonorBallot = {
  gameId: 1124,
  votes: 1,
  honoredPuuids: [],
  eligibleAllies: [
    { puuid: 'p-ksante', summonerId: 1134, championId: 897, championName: "K'Sante", role: 'TOP', summonerName: 'Rafa', bot: false },
    { puuid: 'p-viego', summonerId: 1132, championId: 234, championName: 'Viego', role: 'JUNGLE', summonerName: 'Victor', bot: false },
  ],
};

it('honra um aliado da cédula e mostra quem foi honrado', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  const honor = within(panel().getByRole('region', { name: 'Honra' }));
  expect(honor.getByText('Topo')).toBeInTheDocument();
  fireEvent.click(honor.getByRole('button', { name: 'Honrar Victor' }));
  expect(request().command).toEqual({ type: 'honorPlayer', puuid: 'p-viego' });
  expect(honor.getByRole('button', { name: 'Honrar Rafa' })).toBeDisabled();
  await respond();
  // A LCU aceitou, mas o voto só vale quando a cédula traz o aliado.
  expect(panel().getByRole('status')).toHaveTextContent('Voto enviado. Esperando a confirmação da Riot…');
  act(() => socket().receive({ type: 'honorBallot', honorBallot: { ...ballot, honoredPuuids: ['p-viego'] } }));
  expect(panel().getByRole('status')).toHaveTextContent('Você honrou Victor.');
  act(() => { vi.advanceTimersByTime(HONOR_CONFIRM_MS); });
  expect(panel().getByRole('status')).toHaveTextContent('Você honrou Victor.');
  expect(panel().queryByRole('region', { name: 'Honra' })).not.toBeInTheDocument();
  expect(panel().getByRole('button', { name: 'Voltar ao lobby' })).toBeEnabled();
});

it('avisa quando a Riot não confirma o voto e deixa tentar de novo', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.click(panel().getByRole('button', { name: 'Honrar Rafa' }));
  await respond();
  act(() => { vi.advanceTimersByTime(HONOR_CONFIRM_MS); });
  expect(panel().getByRole('alert')).toHaveTextContent('A Riot não confirmou o voto em Rafa.');
  expect(panel().getByRole('button', { name: 'Honrar Rafa' })).toBeEnabled();
});

it('pula a honra', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.click(panel().getByRole('button', { name: 'Pular' }));
  expect(request().command).toEqual({ type: 'skipHonor' });
  await respond();
  expect(panel().getByRole('status')).toHaveTextContent('Você pulou a honra desta partida.');
});

it('mostra o erro do voto e deixa votar de novo', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.click(panel().getByRole('button', { name: 'Honrar Rafa' }));
  await respond({ status: 'error', code: 'lcu', message: 'voto recusado' });
  expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível honrar Rafa: voto recusado');
  expect(panel().getByRole('button', { name: 'Honrar Rafa' })).toBeEnabled();
});

it('cédula com alguém já honrado mostra só o resultado, e a cédula fechada some', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: { ...ballot, honoredPuuids: ['p-ksante'] } });
  expect(panel().getByRole('status')).toHaveTextContent('Você honrou Rafa.');
  act(() => socket().receive({ type: 'honorBallot', honorBallot: null }));
  expect(panel().queryByRole('status')).not.toBeInTheDocument();
  expect(panel().queryByRole('region', { name: 'Honra' })).not.toBeInTheDocument();
});

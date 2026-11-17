import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { GameResult } from '../bindings/GameResult';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testState } from '../test/lobby';
import { testGameResult } from '../test/gameResult';

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

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('mostra vitória, fila, duração e PDL da ranqueada', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  expect(panel().getByRole('heading', { name: 'Vitória' })).toBeInTheDocument();
  expect(panel().getByText('Ranqueada Solo/Duo · 25:51')).toBeInTheDocument();
  expect(panel().getByLabelText('Mudança de elo')).toHaveTextContent('+64 PDL · Mestre · 868 PDL');
});

it('mostra o resumo do jogador logado', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  const summary = within(panel().getByRole('region', { name: 'Seu desempenho' }));
  const me = testGameResult.teams[0]!.players.find(p => p.puuid === testGameResult.localPuuid)!;
  expect(summary.getByText(`${me.kills} / ${me.deaths} / ${me.assists}`)).toBeInTheDocument();
  expect(summary.getByText(me.gold.toLocaleString('pt-BR'))).toBeInTheDocument();
  expect(summary.getByText(me.damageToChampions.toLocaleString('pt-BR'))).toBeInTheDocument();
  expect(summary.getByText(String(me.visionScore))).toBeInTheDocument();
});

it('mostra o placar dos dois times, com o jogador destacado', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  const tables = panel().getAllByRole('table');
  expect(tables).toHaveLength(2);
  expect(within(tables[0]!).getByText('Seu time · Vitória')).toBeInTheDocument();
  expect(within(tables[1]!).getByText('Time inimigo · Derrota')).toBeInTheDocument();
  // Cabeçalho mais cinco jogadores em cada time.
  expect(within(tables[0]!).getAllByRole('row')).toHaveLength(6);
  expect(within(tables[1]!).getAllByRole('row')).toHaveLength(6);
  const current = within(tables[0]!).getAllByRole('row').filter(r => r.getAttribute('aria-current') === 'true');
  expect(current).toHaveLength(1);
  expect(current[0]).toHaveTextContent('fake-name-0001');
});

it('joga de novo pela tela de resultado', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  fireEvent.click(panel().getByRole('button', { name: 'Jogar de novo' }));
  expect(request().command).toEqual({ type: 'playAgain' });
});

it('troca a espera pelo resultado quando ele chega', () => {
  render(<App />);
  hello({ gameflowPhase: 'WaitingForStats' });
  expect(panel().getByText('Calculando o resultado…')).toBeInTheDocument();
  act(() => socket().receive({ type: 'gameResult', gameResult: testGameResult }));
  expect(panel().getByRole('heading', { name: 'Vitória' })).toBeInTheDocument();
  act(() => socket().receive({ type: 'gameResult', gameResult: null }));
  expect(panel().queryByRole('heading', { name: 'Vitória' })).not.toBeInTheDocument();
});

it('remake não mostra vitória nem PDL', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: { ...testGameResult, result: 'remake', rankedChange: null } });
  expect(panel().getByRole('heading', { name: 'Remake' })).toBeInTheDocument();
  expect(panel().queryByLabelText('Mudança de elo')).not.toBeInTheDocument();
  expect(panel().getByText('Seu time')).toBeInTheDocument();
});

it('funciona com um resultado incompleto, sem fila, sem PDL e sem o jogador logado', () => {
  const partial: GameResult = {
    ...testGameResult,
    queueId: 0, queueName: '', gameMode: 'ARAM', ranked: false, result: 'loss', localPuuid: '', rankedChange: null,
    teams: [{ teamId: 100, playerTeam: false, win: true, players: [{
      ...testGameResult.teams[1]!.players[0]!, gameName: '', tagLine: '', items: [0, 0, 0, 0, 0, 0, 0],
    }] }],
  };
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: partial });
  expect(panel().getByRole('heading', { name: 'Derrota' })).toBeInTheDocument();
  expect(panel().getByText('ARAM · 25:51')).toBeInTheDocument();
  expect(panel().queryByRole('region', { name: 'Seu desempenho' })).not.toBeInTheDocument();
  expect(panel().getAllByRole('row')).toHaveLength(2);
});

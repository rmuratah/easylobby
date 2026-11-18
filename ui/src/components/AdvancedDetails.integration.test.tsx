import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { GameResult } from '../bindings/GameResult';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testRuneCatalog } from '../test/events';
import { testGameResult } from '../test/gameResult';
import { testState } from '../test/lobby';

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
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
const tab = (name: string) => panel().getByRole('button', { name });

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('mostra as estatísticas por categoria, com o melhor valor em destaque (#586)', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  expect(tab('Placar')).toHaveAttribute('aria-pressed', 'true');
  fireEvent.click(tab('Estatísticas'));
  const table = within(panel().getByRole('table', { name: 'Estatísticas da partida' }));
  // Cabeçalho: os dez jogadores.
  expect(table.getAllByRole('columnheader').filter(th => th.closest('thead'))).toHaveLength(11);
  const row = table.getByRole('row', { name: /^Dano total a campeões/ });
  // Katarina (36.963) tem o maior dano a campeões.
  const best = within(row).getByText('36.963');
  expect(best).toHaveClass('text-accent-ink');
  expect(within(row).getByText('14.268')).not.toHaveClass('text-accent-ink');
  // Pings começam fechados; abrir mostra as linhas.
  expect(table.queryByRole('row', { name: /^A caminho/ })).not.toBeInTheDocument();
  fireEvent.click(table.getByRole('button', { name: 'Pings' }));
  expect(table.getByRole('row', { name: /^A caminho/ })).toBeInTheDocument();
  // Fechar uma categoria esconde as linhas.
  fireEvent.click(table.getByRole('button', { name: 'Combate' }));
  expect(table.queryByRole('row', { name: /^Maior multiabate/ })).not.toBeInTheDocument();
});

it('mostra um gráfico por estatística (#586)', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  fireEvent.click(tab('Gráficos'));
  expect(panel().getByText('Combate · K / D / A')).toBeInTheDocument();
  fireEvent.click(panel().getByRole('button', { name: 'Ouro recebido' }));
  expect(panel().getByText('Receita · Ouro recebido')).toBeInTheDocument();
  expect(panel().getByText('16.079')).toBeInTheDocument();
});

it('mostra as runas com as estatísticas do fim da partida (#586)', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult, runeCatalog: testRuneCatalog });
  fireEvent.click(tab('Runas'));
  const mine = within(panel().getByRole('region', { name: 'Runas de fake-name-0001' }));
  expect(mine.getByText('Eletrocutar')).toBeInTheDocument();
  expect(mine.getByText('Total de dano causado: 1.419')).toBeInTheDocument();
});

it('sem runas na partida (ARAM: Desordem), não mostra a aba Runas (#586)', () => {
  const noRunes: GameResult = {
    ...testGameResult,
    teams: testGameResult.teams.map(team => ({ ...team, advanced: team.advanced.map(stats => ({ ...stats, perks: [] })) })),
  };
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: noRunes });
  expect(tab('Estatísticas')).toBeInTheDocument();
  expect(panel().queryByRole('button', { name: 'Runas' })).not.toBeInTheDocument();
});

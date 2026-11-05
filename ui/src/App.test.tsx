import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import App from './App';
import { useAgent } from './agent/useAgent';
import type { LcuStatus } from './bindings/LcuStatus';

vi.mock('./agent/useAgent');
function connected(lcu: LcuStatus) {
  vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null, state: { lcu, summoner: null, gameflowPhase: null, readyCheck: null, lobby: null, matchmaking: null }, send: vi.fn() });
}
describe('App', () => {
  it('mostra o título e a espera pelo agente', () => {
    vi.mocked(useAgent).mockReturnValue({ status: 'connecting', error: null, state: null, send: vi.fn() });
    render(<App />);
    expect(screen.getByRole('heading', { name: 'easylobby' })).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Conectando ao agente');
  });
  it('distingue agente conectado de LoL fechado', () => {
    connected({ state: 'retrying', attempt: 1, retryInMs: 1000, clientNotRunning: true, error: 'fechado' });
    render(<App />);
    expect(screen.getByRole('status')).toHaveTextContent('Abra o League of Legends');
  });
  it('mostra conexão com o LoL após receber estado', () => {
    connected({ state: 'connected', session: 1 });
    render(<App />);
    expect(screen.getByRole('status')).toHaveTextContent('Aguardando login no League of Legends');
  });
  it('apresenta orientação de versão incompatível', () => {
    vi.mocked(useAgent).mockReturnValue({ status: 'incompatible', error: 'Atualize o aplicativo e o agente para versões compatíveis.', state: null, send: vi.fn() });
    render(<App />);
    expect(screen.getByRole('status')).toHaveTextContent('Versões incompatíveis');
    expect(screen.getByRole('alert')).toHaveTextContent('Atualize');
  });
});

describe('invocador logado', () => {
  const summoner = { gameName: 'Victor', tagLine: 'BR1', displayName: 'Antigo', summonerLevel: 122, profileIconId: 1, puuid: 'fake', summonerId: 1 };
  function loggedIn() {
    vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null, state: { lcu: { state: 'connected', session: 1 }, summoner, gameflowPhase: null, readyCheck: null, lobby: null, matchmaking: null }, send: vi.fn() });
  }
  it('mostra Riot ID e nível e remove os dados ao perder a conexão', () => {
    loggedIn();
    const { rerender } = render(<App />);
    expect(screen.getByRole('heading', { name: 'Victor#BR1' })).toBeInTheDocument();
    expect(screen.getByText('Nível 122')).toBeInTheDocument();
    vi.mocked(useAgent).mockReturnValue({ status: 'reconnecting', state: null, error: null, send: vi.fn() });
    rerender(<App />);
    expect(screen.queryByRole('region', { name: 'Invocador logado' })).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Reconectando');
    loggedIn();
    rerender(<App />);
    expect(screen.getByText('Nível 122')).toBeInTheDocument();
    connected({ state: 'connected', session: 1 });
    rerender(<App />);
    expect(screen.queryByText('Nível 122')).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Aguardando login');
  });
  it('oculta dados antigos quando o LoL desconecta', () => {
    vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null, state: { lcu: { state: 'starting' }, summoner, gameflowPhase: null, readyCheck: null, lobby: null, matchmaking: null }, send: vi.fn() });
    render(<App />);
    expect(screen.queryByText('Nível 122')).not.toBeInTheDocument();
  });
});

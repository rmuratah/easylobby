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
    expect(screen.getByRole('status')).toHaveTextContent('Conectado ao League of Legends');
  });
  it('apresenta orientação de versão incompatível', () => {
    vi.mocked(useAgent).mockReturnValue({ status: 'incompatible', error: 'Atualize o aplicativo e o agente para versões compatíveis.', state: null, send: vi.fn() });
    render(<App />);
    expect(screen.getByRole('status')).toHaveTextContent('Versões incompatíveis');
    expect(screen.getByRole('alert')).toHaveTextContent('Atualize');
  });
});

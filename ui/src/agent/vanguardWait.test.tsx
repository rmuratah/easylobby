import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { Vanguard } from '../bindings/Vanguard';
import { makeState } from '../test/state';
import type { ConnectionSnapshot } from './client';
import { useAgent } from './useAgent';
import { VANGUARD_WAIT_MS } from './vanguardWait';

vi.mock('./useAgent');

const summoner = { gameName: 'Victor', tagLine: 'BR1', displayName: 'Antigo', summonerLevel: 122, profileIconId: 1, puuid: 'fake', summonerId: 1 };
const open = (vanguard: Vanguard | null): ConnectionSnapshot =>
  ({ status: 'connected', error: null, state: makeState({ summoner, vanguard }) });
const refused = open({ connected: false, state: 'ERROR', status: 216 });
const closed: ConnectionSnapshot = { status: 'connected', error: null,
  state: makeState({ lcu: { state: 'retrying', attempt: 1, retryInMs: 1000, clientNotRunning: true, error: 'fechado' } }) };

function show(connection: ConnectionSnapshot) {
  vi.mocked(useAgent).mockReturnValue({ ...connection, send: vi.fn() });
}

const notice = () => screen.queryByRole('status', { name: 'Espera do Vanguard' });

describe('espera do VAN 216 (#668)', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 10, 24, 21, 22, 18));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('depois que o LoL fecha, a tela de LoL fechado manda esperar até o horário', () => {
    show(refused);
    const { rerender } = render(<App />);
    expect(screen.getByRole('heading', { name: 'Espere até as 21:53 para abrir o LoL de novo' })).toBeInTheDocument();
    show(closed);
    rerender(<App />);
    expect(notice()).toHaveTextContent('Espere até as 21:53 para abrir o LoL');
    expect(notice()).toHaveTextContent('erro VAN 216');
    // O botão continua: quem abrir antes do horário só recomeça a espera.
    expect(screen.getByRole('button', { name: 'Abrir o League of Legends' })).toBeEnabled();
    act(() => { vi.advanceTimersByTime(VANGUARD_WAIT_MS); });
    expect(notice()).not.toBeInTheDocument();
  });

  it('a espera vale depois de fechar e abrir o app', () => {
    show(refused);
    render(<App />).unmount();
    act(() => { vi.advanceTimersByTime(10 * 60_000); });
    show(closed);
    render(<App />);
    expect(notice()).toHaveTextContent('Espere até as 21:53 para abrir o LoL');
  });

  it('uma recusa nova recomeça os 30 minutos', () => {
    show(refused);
    const { rerender } = render(<App />);
    show(closed);
    rerender(<App />);
    act(() => { vi.advanceTimersByTime(10 * 60_000); });
    show(refused);
    rerender(<App />);
    show(closed);
    rerender(<App />);
    expect(notice()).toHaveTextContent('Espere até as 22:03 para abrir o LoL');
  });

  it('o Vanguard conectado apaga a espera', () => {
    show(refused);
    const { rerender } = render(<App />);
    show(open({ connected: true, state: 'CONNECTED', status: 0 }));
    rerender(<App />);
    show(closed);
    rerender(<App />);
    expect(notice()).not.toBeInTheDocument();
  });

  it('com outro código, a tela de LoL fechado não manda esperar', () => {
    show(open({ connected: false, state: 'ERROR', status: 7003 }));
    const { rerender } = render(<App />);
    show(closed);
    rerender(<App />);
    expect(screen.getByRole('button', { name: 'Abrir o League of Legends' })).toBeInTheDocument();
    expect(notice()).not.toBeInTheDocument();
  });
});

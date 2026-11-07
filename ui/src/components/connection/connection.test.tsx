import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import App from '../../App';
import type { ConnectionSnapshot } from '../../agent/client';
import { useAgent } from '../../agent/useAgent';
import type { CommandResult } from '../../bindings/CommandResult';
import type { LcuStatus } from '../../bindings/LcuStatus';
import { connectionView } from '../../screens/connection';
import { makeState } from '../../test/state';

vi.mock('../../agent/useAgent');

const summoner = { gameName: 'Victor', tagLine: 'BR1', displayName: 'Antigo', summonerLevel: 122, profileIconId: 1, puuid: 'fake', summonerId: 1 };
const withLcu = (lcu: LcuStatus, extra: Parameters<typeof makeState>[0] = {}): ConnectionSnapshot =>
  ({ status: 'connected', error: null, state: makeState({ lcu, ...extra }) });
const closed = withLcu({ state: 'retrying', attempt: 1, retryInMs: 1000, clientNotRunning: true, error: 'fechado' });

function show(connection: ConnectionSnapshot, send = vi.fn<() => Promise<CommandResult>>()) {
  vi.mocked(useAgent).mockReturnValue({ ...connection, send });
  render(<App />);
  return send;
}

describe('estados de conexão', () => {
  it.each<[string, ConnectionSnapshot, string, string | null]>([
    ['agente conectando', { status: 'connecting', error: null, state: null }, 'Conectando ao agente…', 'Conectando ao agente'],
    ['agente caiu', { status: 'reconnecting', error: null, state: null }, 'Reconectando ao agente…', 'Reconectando ao agente'],
    ['versão incompatível', { status: 'incompatible', error: null, state: null }, 'Versões incompatíveis', 'Versões incompatíveis'],
    ['LoL abrindo', withLcu({ state: 'starting' }), 'Aguardando o League of Legends…', 'Procurando o League of Legends'],
    ['LoL fechado', closed, 'Abra o League of Legends para continuar', 'O League of Legends está fechado'],
    ['LoL sem responder', withLcu({ state: 'retrying', attempt: 2, retryInMs: 1000, clientNotRunning: false, error: 'senha' }),
      'Tentando reconectar ao League of Legends…', 'Tentando reconectar ao League of Legends'],
    ['LoL caiu', withLcu({ state: 'disconnected', reason: 'fechado' }), 'Conexão com o League of Legends interrompida', 'Conexão com o League of Legends interrompida'],
    ['sem login', withLcu({ state: 'connected', session: 1 }), 'Aguardando login no League of Legends…', 'Aguardando o login'],
    ['pronto', withLcu({ state: 'connected', session: 1 }, { summoner }), 'Conectado ao League of Legends', null],
  ])('%s: barra e tela próprias', (_, connection, label, title) => {
    expect(connectionView(connection).label).toBe(label);
    show(connection);
    expect(screen.getByRole('status')).toHaveTextContent(label);
    if (title) {
      expect(screen.queryByRole('navigation', { name: 'Funcionalidades' })).not.toBeInTheDocument();
      expect(screen.queryByRole('complementary', { name: 'Amigos e conversas' })).not.toBeInTheDocument();
      expect(screen.getByRole('heading', { level: 2, name: title })).toBeInTheDocument();
    } else {
      expect(screen.getByRole('navigation', { name: 'Funcionalidades' })).toBeInTheDocument();
      expect(screen.getByRole('complementary', { name: 'Amigos e conversas' })).toBeInTheDocument();
    }
  });

  it('mostra o erro do agente na tela de espera', () => {
    show({ status: 'incompatible', error: 'Atualize o aplicativo e o agente.', state: null });
    expect(screen.getByRole('alert')).toHaveTextContent('Atualize o aplicativo');
  });

  it('com o LoL fechado, abre o jogo pelo agente', async () => {
    const send = show(closed, vi.fn(async () => ({ status: 'ok' }) as CommandResult));
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Abrir o League of Legends' })); });
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'launchLeague' });
    expect(screen.getByText(/Abrindo o League of Legends/)).toBeInTheDocument();
  });

  it('avisa quando não consegue abrir o LoL e deixa tentar de novo', async () => {
    show(closed, vi.fn(async () => ({ status: 'error', code: 'launchFailed', message: 'Riot Client não encontrado' }) as CommandResult));
    const button = screen.getByRole('button', { name: 'Abrir o League of Legends' });
    await act(async () => { fireEvent.click(button); });
    expect(screen.getByRole('alert')).toHaveTextContent('Abra pelo Riot Client');
    expect(button).toBeEnabled();
  });
});

describe('início', () => {
  it('mostra ícone, Riot ID e nível do invocador', () => {
    show(withLcu({ state: 'connected', session: 1 }, { summoner }));
    const region = screen.getByRole('region', { name: 'Invocador logado' });
    expect(region.querySelector('img')).toHaveAttribute('src', expect.stringMatching(/\/assets\/profileIcon\/1$/));
    expect(screen.getByRole('heading', { name: 'Victor#BR1' })).toBeInTheDocument();
    expect(screen.getByText('Nível 122')).toBeInTheDocument();
  });

  it('explica a fase que ainda não tem tela e aponta o client oficial', () => {
    show(withLcu({ state: 'connected', session: 1 }, { summoner, gameflowPhase: 'InProgress' }));
    expect(screen.getByRole('heading', { name: 'Partida em andamento.' })).toBeInTheDocument();
    expect(screen.getByText(/Esta fase ainda não tem tela/)).toHaveTextContent('Abrir client oficial');
  });
});

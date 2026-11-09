import { testLobby, testState } from './test/lobby';
import { testFriend, testMatchmaking } from './test/events';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import App from './App';
import { useAgent } from './agent/useAgent';
import type { LcuStatus } from './bindings/LcuStatus';
import { makeState } from './test/state';

vi.mock('./agent/useAgent');
function connected(lcu: LcuStatus) {
  vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null, state: makeState({ lcu }), send: vi.fn() });
}
describe('App', () => {
  it('mostra o título e a espera pelo agente', () => {
    vi.mocked(useAgent).mockReturnValue({ status: 'connecting', error: null, state: null, send: vi.fn() });
    render(<App />);
    expect(screen.queryByRole('complementary', { name: 'Navegação do aplicativo' })).not.toBeInTheDocument();
    expect(screen.queryByRole('complementary', { name: 'Amigos e convites' })).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Conectando ao agente');
  });
  it('distingue agente conectado de LoL fechado', () => {
    connected({ state: 'retrying', attempt: 1, retryInMs: 1000, clientNotRunning: true, error: 'fechado' });
    render(<App />);
    expect(screen.getByRole('status')).toHaveTextContent('Abra o League of Legends');
    expect(screen.getByRole('button', { name: 'Abrir o League of Legends' })).toBeVisible();
    expect(screen.queryByRole('complementary', { name: 'Navegação do aplicativo' })).not.toBeInTheDocument();
    expect(screen.queryByRole('complementary', { name: 'Amigos e convites' })).not.toBeInTheDocument();
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
  it('deixa as margens em volta das áreas arrastarem a janela (#299)', () => {
    connected({ state: 'connected', session: 1 });
    const { container } = render(<App />);
    for (const area of ['.app-shell', '.app-body', '.app-main-column']) {
      expect(container.querySelector(area)).toHaveAttribute('data-tauri-drag-region');
    }
    // Só o próprio contorno arrasta: o conteúdo continua clicável.
    expect(screen.getByRole('main')).not.toHaveAttribute('data-tauri-drag-region');
  });
});

describe('invocador logado', () => {
  const summoner = { gameName: 'Victor', tagLine: 'BR1', displayName: 'Antigo', summonerLevel: 122, profileIconId: 1, puuid: 'fake', summonerId: 1 };
  function loggedIn() {
    vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null, state: makeState({ summoner }), send: vi.fn() });
  }
  it('mostra as barras laterais somente depois do login', () => {
    loggedIn();
    render(<App />);
    expect(screen.getByRole('complementary', { name: 'Navegação do aplicativo' })).toBeVisible();
    expect(screen.getByRole('complementary', { name: 'Amigos e convites' })).toBeVisible();
  });
  it('mostra Riot ID e nível e remove os dados ao perder a conexão', () => {
    loggedIn();
    const { rerender } = render(<App />);
    expect(screen.getByRole('heading', { name: 'Victor#BR1' })).toBeInTheDocument();
    expect(screen.getByText('Nível 122')).toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'Presença indisponível' })).toBeInTheDocument();
    expect(screen.getByText('Solo/Duo').nextElementSibling).toHaveTextContent('Indisponível');
    expect(screen.getByText('Flex').nextElementSibling).toHaveTextContent('Indisponível');
    expect(screen.getByRole('article', { name: 'Área reservada' })).toHaveTextContent('futura funcionalidade');
    expect(screen.getByRole('article', { name: 'Sem conteúdo' })).toHaveTextContent('reservado no momento');
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
    vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null, state: makeState({ lcu: { state: 'starting' }, summoner }), send: vi.fn() });
    render(<App />);
    expect(screen.queryByText('Nível 122')).not.toBeInTheDocument();
  });
});

describe('telas confirmadas pelo agente', () => {
  const send = vi.fn(() => new Promise<never>(() => {}));
  function snapshot(overrides: Parameters<typeof makeState>[0] = {}) {
    vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null,
      state: makeState({ ...testState, ...overrides }), send });
  }
  it('chama o destino do fluxo de Jogar sem lobby e de Grupo com lobby', () => {
    snapshot();
    const { rerender } = render(<App />);
    const navigation = screen.getByRole('navigation', { name: 'Funcionalidades' });
    expect(navigation).toHaveTextContent('Jogar');
    expect(navigation).not.toHaveTextContent('Grupo');
    snapshot({ lobby: testLobby, gameflowPhase: 'Lobby' });
    rerender(<App />);
    expect(navigation).toHaveTextContent('Grupo');
    expect(navigation).not.toHaveTextContent('Jogar');
  });
  it('mostra a área reservada no Início sem lobby e cria o lobby pelo Jogar', () => {
    snapshot();
    render(<App />);
    expect(screen.getByRole('article', { name: 'Área reservada para atualização' })).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Criar lobby' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Jogar' }));
    expect(screen.queryByRole('article', { name: 'Área reservada para atualização' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Criar lobby' })).toBeVisible();
  });
  it('preserva criar lobby pendente ao receber o lobby', () => {
    send.mockClear();
    snapshot();
    const { rerender } = render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Jogar' }));
    fireEvent.click(screen.getByRole('button', { name: 'Criar lobby' }));
    expect(screen.getByRole('button', { name: 'Criar lobby' })).toBeDisabled();
    snapshot({ lobby: testLobby, gameflowPhase: 'Lobby' });
    rerender(<App />);
    expect(screen.getByRole('button', { name: 'Sair do lobby' })).toBeDisabled();
    expect(send).toHaveBeenCalledTimes(1);
  });
  it('preserva busca pendente entre Lobby, Início e Fila e volta após reconectar', () => {
    send.mockClear();
    snapshot({ lobby: testLobby, gameflowPhase: 'Lobby' });
    const { rerender } = render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar busca' }));
    fireEvent.click(screen.getByRole('button', { name: 'Início' }));
    snapshot({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking });
    rerender(<App />);
    expect(screen.getByRole('button', { name: 'Cancelar busca' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Trocar fila' })).not.toBeInTheDocument();
    expect(screen.getByLabelText('Tempo na fila')).toBeVisible();
    expect(send).toHaveBeenCalledTimes(1);
    vi.mocked(useAgent).mockReturnValue({ status: 'reconnecting', state: null, error: null, send });
    rerender(<App />);
    expect(screen.queryByRole('button', { name: 'Cancelar busca' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Abrir client oficial' })).toBeDisabled();
    snapshot({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking });
    rerender(<App />);
    expect(screen.getByRole('button', { name: 'Cancelar busca' })).toBeEnabled();
  });
  it('destaca partida encontrada mesmo antes da fase e durante navegação', () => {
    snapshot({ lobby: testLobby, gameflowPhase: 'Lobby' });
    const { rerender } = render(<App />);
    fireEvent.click(screen.getByRole('button', { name: 'Início' }));
    snapshot({ lobby: testLobby, gameflowPhase: 'Lobby', matchmaking: { ...testMatchmaking, searchState: 'Found' } });
    rerender(<App />);
    expect(screen.getByRole('alert')).toHaveTextContent('Partida encontrada');
    expect(screen.queryByRole('button', { name: 'Iniciar busca' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Trocar fila' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Início' }));
    expect(screen.getByRole('alert')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Abrir client oficial' })).toBeEnabled();
    snapshot({ gameflowPhase: 'ChampSelect', matchmaking: testMatchmaking });
    rerender(<App />);
    expect(screen.getByText('Carregando a seleção de campeões…')).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Cancelar busca' })).not.toBeInTheDocument();
  });
});

describe('perfil do amigo (#255)', () => {
  const summoner = { gameName: 'Victor', tagLine: 'BR1', displayName: '', summonerLevel: 122, profileIconId: 1, puuid: 'fake', summonerId: 1 };
  it('abre ao clicar no amigo da barra lateral e volta para a página de antes', () => {
    const send = vi.fn(() => Promise.resolve({ status: 'ok' as const }));
    vi.mocked(useAgent).mockReturnValue({ status: 'connected', error: null, state: makeState({ summoner, friends: [testFriend] }), send });
    render(<App />);
    fireEvent.click(screen.getByRole('button', { name: /^Ver perfil de Amigo/ }));
    expect(send).toHaveBeenCalledWith({ type: 'openPlayerProfile', puuid: testFriend.puuid });
    expect(screen.getByRole('region', { name: 'Perfil do amigo' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Perfil' })).toHaveAttribute('aria-current', 'page');
    fireEvent.click(screen.getByRole('button', { name: 'Voltar' }));
    expect(screen.queryByRole('region', { name: 'Perfil do amigo' })).toBeNull();
    expect(screen.getByRole('region', { name: 'Início' })).toBeVisible();
  });
});

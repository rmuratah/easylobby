import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testMatchmaking } from '../test/events';
import { testReadyLobby as testLobby, testState } from '../test/lobby';

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
const panel = () => within(screen.getByRole('region', { name: 'Busca de partida' }));
const lobbyPanel = () => within(screen.getByRole('region', { name: 'Lobby' }));
const button = (name: string) => panel().getByRole('button', { name });
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: request().id, result }));
}

// Estes testes verificam também as guardas dos painéis montados fora da tela ativa.
beforeEach(() => { configure({ defaultHidden: true }); vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.useRealTimers(); vi.unstubAllGlobals(); });

describe('matchmaking pelo /ws', () => {
  it('inicia uma vez e mantém o agente como fonte de verdade', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    act(() => { fireEvent.click(button('Iniciar busca')); fireEvent.click(button('Iniciar busca')); });
    expect(request().command).toEqual({ type: 'startQueue' });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(panel().getByText('Fora da fila', { selector: 'strong' })).toBeInTheDocument();
    expect(button('Iniciar busca')).toBeDisabled();
    act(() => {
      socket().receive({ type: 'gameflowPhase', phase: 'Matchmaking' });
      socket().receive({ type: 'matchmaking', matchmaking: testMatchmaking });
    });
    expect(panel().getByText('Buscando partida', { selector: 'strong' })).toBeInTheDocument();
    await respond();
    expect(button('Cancelar busca')).toBeEnabled();
    expect(lobbyPanel().queryByRole('combobox', { name: 'Fila' })).not.toBeInTheDocument();
    expect(lobbyPanel().getByRole('combobox', { name: 'Posição primária' })).toBeDisabled();
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
    expect(lobbyPanel().queryByRole('button', { name: 'Sair do lobby' })).not.toBeInTheDocument();
    expect(lobbyPanel().getByRole('combobox', { name: 'Posição primária' })).toBeDisabled();
  });

  it('sincroniza e limpa o cronômetro nas transições', () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking });
    expect(panel().getByLabelText('Tempo na fila')).toHaveTextContent('0:31');
    act(() => vi.advanceTimersByTime(3000));
    expect(panel().getByLabelText('Tempo na fila')).toHaveTextContent('0:34');
    act(() => socket().receive({ type: 'matchmaking', matchmaking: { ...testMatchmaking, timeInQueue: 50 } }));
    expect(panel().getByLabelText('Tempo na fila')).toHaveTextContent('0:50');
    act(() => socket().receive({ type: 'matchmaking', matchmaking: null }));
    expect(panel().queryByLabelText('Tempo na fila')).not.toBeInTheDocument();
    act(() => vi.advanceTimersByTime(5000));
    expect(panel().getByText('Fora da fila', { selector: 'strong' })).toBeInTheDocument();
  });

  it('cancela uma vez e só remove a busca após confirmação do agente', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking });
    act(() => { fireEvent.click(button('Cancelar busca')); fireEvent.click(button('Cancelar busca')); });
    expect(request().command).toEqual({ type: 'stopQueue' });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(panel().getByText('Buscando partida', { selector: 'strong' })).toBeInTheDocument();
    act(() => {
      socket().receive({ type: 'matchmaking', matchmaking: null });
      socket().receive({ type: 'gameflowPhase', phase: 'Lobby' });
    });
    await respond();
    expect(panel().getByText('Fora da fila', { selector: 'strong' })).toBeInTheDocument();
  });

  it('mostra baixa prioridade, estimativa e erros', () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: {
      ...testMatchmaking, searchState: 'Error', estimatedQueueTime: 95, lowPriorityTimeRemaining: 120,
      errors: [{ errorType: 'QUEUE_DODGER', message: '', penaltyTimeRemaining: 300 }],
    } });
    expect(panel().getByText('1:35', { selector: 'strong' })).toBeInTheDocument();
    expect(panel().getByText(/Baixa prioridade: 2:00/)).toBeInTheDocument();
    expect(within(panel().getByRole('list', { name: 'Erros da busca' })).getByText(/QUEUE_DODGER.*5:00/)).toBeInTheDocument();
  });

  it('cancela a busca parada por punição mesmo fora da fase Matchmaking', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby', matchmaking: {
      ...testMatchmaking, searchState: 'Error', errors: [{ errorType: 'QUEUE_DODGER', message: '', penaltyTimeRemaining: 300 }],
    } });
    fireEvent.click(button('Cancelar busca'));
    expect(request().command).toEqual({ type: 'stopQueue' });
    await respond();
  });

  it('respeita lobby, liderança, prontidão e fase', () => {
    render(<App />);
    hello({ gameflowPhase: 'Lobby' });
    expect(button('Iniciar busca')).toBeDisabled();
    hello({ lobby: { ...testLobby, isLeader: false }, gameflowPhase: 'Lobby' });
    expect(panel().getByText(/Somente o líder/)).toBeInTheDocument();
    hello({ lobby: { ...testLobby, canStartActivity: false }, gameflowPhase: 'Lobby' });
    expect(panel().getByText(/ainda não está pronto/)).toBeInTheDocument();
    hello({ lobby: testLobby, gameflowPhase: 'None' });
    expect(button('Iniciar busca')).toBeDisabled();
    hello({ lobby: testLobby, gameflowPhase: 'ReadyCheck', matchmaking: { ...testMatchmaking, searchState: 'Found' } });
    expect(button('Cancelar busca')).toBeDisabled();
  });

  it('mostra quem ainda não voltou ao lobby depois da partida (#346)', () => {
    render(<App />);
    const away = { ...testLobby.members[0], puuid: 'amigo', summonerId: 2, ready: false, gameName: 'Games e Musica', tagLine: 'BR1', isLeader: false };
    hello({ lobby: { ...testLobby, canStartActivity: false, members: [...testLobby.members, away] }, gameflowPhase: 'Lobby' });
    expect(button('Iniciar busca')).toBeDisabled();
    expect(panel().getByText('Os jogadores não estão prontos:')).toBeInTheDocument();
    const reasons = within(panel().getByRole('list', { name: 'Motivos da fila bloqueada' })).getAllByRole('listitem');
    expect(reasons.map(r => r.textContent)).toEqual(['Games e Musica#BR1 ainda não voltou ao lobby.']);
    expect(lobbyPanel().getAllByText('Ainda não voltou ao lobby')).toHaveLength(1);
    // Ao voltar, o aviso some e a busca libera.
    hello({ lobby: { ...testLobby, members: [...testLobby.members, { ...away, ready: true }] }, gameflowPhase: 'Lobby' });
    expect(lobbyPanel().queryByText('Ainda não voltou ao lobby')).not.toBeInTheDocument();
    expect(panel().queryByText('Os jogadores não estão prontos:')).not.toBeInTheDocument();
    expect(button('Iniciar busca')).toBeEnabled();
  });

  it('mostra o motivo da fila bloqueada que a LCU mandou (#227)', () => {
    render(<App />);
    const restrictions = [
      { code: 'TeamSizeRestriction', message: 'Esta fila não aceita grupos deste tamanho.', puuids: [] },
      { code: 'RestricaoNova', message: null, puuids: [] },
    ];
    hello({ lobby: { ...testLobby, canStartActivity: false, restrictions }, gameflowPhase: 'Lobby' });
    expect(button('Iniciar busca')).toBeDisabled();
    const reasons = within(panel().getByRole('list', { name: 'Motivos da fila bloqueada' })).getAllByRole('listitem');
    expect(reasons.map(r => r.textContent)).toEqual([
      'Esta fila não aceita grupos deste tamanho.',
      'O League of Legends não deixa iniciar a busca com este grupo agora. (RestricaoNova)',
    ]);
    // Com a fila liberada, motivos que sobraram não aparecem.
    hello({ lobby: { ...testLobby, canStartActivity: true, restrictions }, gameflowPhase: 'Lobby' });
    expect(panel().queryByRole('list', { name: 'Motivos da fila bloqueada' })).not.toBeInTheDocument();
    expect(button('Iniciar busca')).toBeEnabled();
  });

  it('trata erro, falha de envio e timeout sem repetir comandos', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    fireEvent.click(button('Iniciar busca'));
    await respond({ status: 'error', code: 'lcu', message: 'Fila indisponível' });
    expect(panel().getByRole('alert')).toHaveTextContent('Fila indisponível');
    socket().send.mockImplementationOnce(() => { throw new Error('envio'); });
    await act(async () => fireEvent.click(button('Iniciar busca')));
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível confirmar');
    fireEvent.click(button('Iniciar busca'));
    const count = socket().send.mock.calls.length;
    await act(async () => vi.advanceTimersByTime(10000));
    expect(panel().getByRole('alert')).toHaveTextContent('Confira a conexão');
    expect(socket().send).toHaveBeenCalledTimes(count);
  });

  it('limpa a tela e descarta resposta ao desconectar', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking });
    fireEvent.click(button('Cancelar busca'));
    const old = socket();
    const id = request().id;
    await act(async () => old.onclose?.());
    expect(panel().getByText(/Aguarde a reconexão/)).toBeInTheDocument();
    expect(panel().queryByLabelText('Tempo na fila')).not.toBeInTheDocument();
    await act(async () => old.receive({ type: 'response', id, result: { status: 'ok' } }));
    expect(panel().queryByText('Cancelamento confirmado')).not.toBeInTheDocument();
  });
});

describe('tela unificada', () => {
  it('mantém o lobby à vista durante a busca, com os cargos travados', () => {
    configure({ defaultHidden: false });
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking });
    expect(screen.getByRole('region', { name: 'Lobby' })).toBeVisible();
    expect(screen.getByRole('region', { name: 'Busca de partida' })).toBeVisible();
    expect(screen.getByRole('combobox', { name: 'Posição primária' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Trocar fila' })).not.toBeInTheDocument();
  });

  it('mostra um botão principal por vez: iniciar fora da fila, cancelar durante a busca', () => {
    configure({ defaultHidden: false });
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    expect(screen.getByRole('button', { name: 'Iniciar busca' })).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Cancelar busca' })).not.toBeInTheDocument();
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking });
    expect(screen.getByRole('button', { name: 'Cancelar busca' })).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Iniciar busca' })).not.toBeInTheDocument();
  });
});

import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import { AGENT_URL } from '../agent/client';
import { queueNames } from '../agent/queues';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { GameflowPhase } from '../bindings/GameflowPhase';
import type { Queue } from '../bindings/Queue';
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
const panel = () => within(screen.getByRole('region', { name: 'Lobby' }));
const button = (name: string) => panel().getByRole('button', { name });
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
async function ok() {
  const id = request().id;
  await act(async () => socket().receive({ type: 'response', id, result: { status: 'ok' } }));
}
function choose(queue: Queue) {
  fireEvent.change(panel().getByRole('combobox', { name: 'Fila' }), { target: { value: queue } });
}
// Estes testes verificam também as guardas dos painéis montados fora da tela ativa.
beforeEach(() => { configure({ defaultHidden: true }); vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.useRealTimers(); vi.unstubAllGlobals(); });

describe('lobby pelo único /ws do App', () => {
  it.each(Object.keys(queueNames) as Queue[])('cria %s sem duplicar e aguarda estado e resposta do agente', async queue => {
    render(<App />);
    expect(Socket.instances).toHaveLength(1);
    expect(socket().url).toBe(AGENT_URL);
    hello();
    expect(panel().getAllByRole('option')).toHaveLength(5);
    choose(queue);
    act(() => { fireEvent.click(button('Criar lobby')); fireEvent.click(button('Criar lobby')); });
    expect(request().command).toEqual({ type: 'createLobby', queue });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(panel().queryByRole('button', { name: 'Sair do lobby' })).not.toBeInTheDocument();
    expect(panel().getByRole('combobox', { name: 'Fila' })).toBeDisabled();
    expect(panel().getByRole('status')).toHaveTextContent('Criando');
    expect(panel().getByText('Você não está em um lobby.')).toBeInTheDocument();
    act(() => {
      socket().receive({ type: 'gameflowPhase', phase: 'Lobby' });
      socket().receive({ type: 'lobby', lobby: { ...testLobby, queue } });
    });
    expect(panel().getByText(queueNames[queue], { selector: 'strong' })).toBeInTheDocument();
    expect(panel().getByRole('status')).toHaveTextContent('Criando');
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('atualizado pelo agente');
    expect(button('Trocar fila')).toBeDisabled();
    expect(button('Sair do lobby')).toBeEnabled();
  });

  it('preserva o perfil e o botão de emergência no mesmo cliente', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    expect(within(screen.getByRole('region', { name: 'Invocador logado' }))
      .getByRole('heading', { name: 'Victor#BR1' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Abrir client oficial' }));
    expect(request().command).toEqual({ type: 'launchUx' });
    await ok();
    expect(screen.getByText('Abertura do client oficial confirmada.')).toBeInTheDocument();
    expect(panel().getByText(queueNames.rankedSolo, { selector: 'strong' })).toBeInTheDocument();
    expect(Socket.instances).toHaveLength(1);
  });

  it('recebe lobby no hello, troca fila, segue eventos externos e sai sem estado otimista', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    expect(panel().getByRole('heading', { name: 'Victor#BR1' })).toBeInTheDocument();
    expect(panel().getByText('Nível 122')).toBeInTheDocument();
    expect(panel().getByRole('img', { name: 'Líder do grupo' })).toBeInTheDocument();
    expect(panel().getByText(/1 \/ 2 jogadores/)).toBeInTheDocument();
    expect(panel().getByRole('combobox', { name: 'Fila' })).toHaveValue('rankedSolo');
    choose('aram');
    fireEvent.click(button('Trocar fila'));
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
    expect(panel().getByText(queueNames.rankedSolo, { selector: 'strong' })).toBeInTheDocument();
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: 'aram', queueId: 450, maxLobbySize: 5 } }));
    await ok();
    expect(panel().getByRole('combobox', { name: 'Fila' })).toHaveValue('aram');
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: null, queueId: 999 } }));
    expect(panel().getByText('Outra fila (ID 999)')).toBeInTheDocument();
    fireEvent.click(button('Sair do lobby'));
    fireEvent.click(button('Sair do lobby'));
    expect(socket().send).toHaveBeenCalledTimes(2);
    expect(request().command).toEqual({ type: 'leaveLobby' });
    expect(panel().getByText('Outra fila (ID 999)')).toBeInTheDocument();
    act(() => {
      socket().receive({ type: 'lobby', lobby: null });
      socket().receive({ type: 'gameflowPhase', phase: 'None' });
    });
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('Saída do lobby confirmada');
    expect(panel().getByText('Você não está em um lobby.')).toBeInTheDocument();
    expect(button('Criar lobby')).toBeEnabled();
  });

  it('impede troca por membro, mas permite sair; reflete mudança de liderança', () => {
    render(<App />);
    hello({ gameflowPhase: 'Lobby', lobby: { ...testLobby, isLeader: false } });
    expect(button('Trocar fila')).toBeDisabled();
    expect(panel().getByRole('combobox', { name: 'Fila' })).toBeDisabled();
    expect(panel().getByText(/Somente o líder/)).toBeInTheDocument();
    choose('aram');
    fireEvent.click(button('Trocar fila'));
    expect(socket().send).not.toHaveBeenCalled();
    expect(button('Sair do lobby')).toBeEnabled();
    act(() => socket().receive({ type: 'lobby', lobby: testLobby }));
    choose('aram');
    expect(button('Trocar fila')).toBeEnabled();
  });

  const blockedPhases: (GameflowPhase | null)[] = [null, 'Unknown', 'Matchmaking', 'ReadyCheck', 'ChampSelect',
    'GameStart', 'InProgress', 'Reconnect', 'EndOfGame', 'CheckedIntoTournament', 'FailedToLaunch',
    'WaitingForStats', 'PreEndOfGame', 'TerminatedInError'];
  it.each(blockedPhases)('bloqueia criar, trocar e sair na fase %s', phase => {
    render(<App />);
    hello({ gameflowPhase: phase });
    expect(button('Criar lobby')).toBeDisabled();
    fireEvent.click(button('Criar lobby'));
    hello({ gameflowPhase: phase, lobby: testLobby });
    expect(button('Trocar fila')).toBeDisabled();
    expect(button('Sair do lobby')).toBeDisabled();
    expect(socket().send).not.toHaveBeenCalled();
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
    expect(button('Sair do lobby')).toBeEnabled();
  });

  it('não confunde canStartActivity com permissão para trocar fila e aguarda login', () => {
    render(<App />);
    hello({ summoner: null });
    expect(button('Criar lobby')).toBeDisabled();
    expect(panel().getByText(/Aguarde o login/)).toBeInTheDocument();
    hello({ lobby: { ...testLobby, canStartActivity: false }, gameflowPhase: 'Lobby' });
    choose('aram');
    expect(button('Trocar fila')).toBeEnabled();
    expect(button('Sair do lobby')).toBeEnabled();
  });

  it.each(['notConnected', 'lcu', 'invalidCommand', 'launchFailed'] as const)('exibe erro %s sem reconectar nem alterar lobby', async code => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choose('aram');
    fireEvent.click(button('Trocar fila'));
    await act(async () => socket().receive({ type: 'response', id: request().id, result: { status: 'error', code, message: 'Fila indisponível' } }));
    expect(panel().getByRole('alert')).toHaveTextContent(code === 'notConnected' ? 'desconectou' : 'Fila indisponível');
    expect(panel().getByText(queueNames.rankedSolo, { selector: 'strong' })).toBeInTheDocument();
    expect(socket().close).not.toHaveBeenCalled();
    expect(Socket.instances).toHaveLength(1);
    expect(button('Trocar fila')).toBeEnabled();
    fireEvent.click(button('Trocar fila'));
    expect(panel().queryByRole('alert')).not.toBeInTheDocument();
    await ok();
  });

  it('trata falha de envio e timeout, ignora resposta tardia e permite tentativa explícita', async () => {
    render(<App />);
    hello();
    socket().send.mockImplementationOnce(() => { throw new Error('falha de envio'); });
    await act(async () => { fireEvent.click(button('Criar lobby')); });
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível confirmar');
    fireEvent.click(button('Criar lobby'));
    const oldId = request().id;
    await act(async () => { vi.advanceTimersByTime(10000); });
    expect(panel().getByRole('alert')).toHaveTextContent('Confira a conexão e o lobby');
    expect(button('Criar lobby')).toBeEnabled();
    fireEvent.click(button('Criar lobby'));
    await act(async () => socket().receive({ type: 'response', id: oldId, result: { status: 'ok' } }));
    expect(panel().getByRole('status')).toHaveTextContent('Criando');
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('atualizado');
  });

  it.each(['agente', 'LoL', 'sessão'] as const)('descarta resultado antigo após mudança de %s e não reenvia', async source => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    fireEvent.click(button('Sair do lobby'));
    const old = socket();
    const oldId = request().id;
    if (source === 'agente') {
      await act(async () => old.onclose?.());
      expect(panel().queryByText(/Victor#BR1/)).not.toBeInTheDocument();
      expect(panel().queryByRole('button')).not.toBeInTheDocument();
      act(() => vi.advanceTimersByTime(1000));
      hello();
    } else {
      act(() => old.receive({ type: 'lcu', status: source === 'LoL'
        ? { state: 'disconnected', reason: 'fechado' } : { state: 'connected', session: 2 } }));
      if (source === 'LoL') {
        expect(panel().queryByText(/Victor#BR1/)).not.toBeInTheDocument();
        hello();
      }
    }
    await act(async () => old.receive({ type: 'response', id: oldId, result: { status: 'ok' } }));
    expect(panel().queryByText('Saída do lobby confirmada.')).not.toBeInTheDocument();
    expect(socket().send).toHaveBeenCalledTimes(source === 'agente' ? 0 : 1);
  });
});

const positionSelect = (name: string) => panel().getByRole('combobox', { name });
// Cargos confirmados de um jogador, como aparecem nas etiquetas da linha dele.
const roles = (name: string) => panel().getByRole('list', { name: `Cargos de ${name}` });
function choosePositions(first: string, second = 'unselected') {
  fireEvent.change(positionSelect('Cargo primário'), { target: { value: first } });
  fireEvent.change(positionSelect('Cargo secundário'), { target: { value: second } });
}

describe('cargos pelo /ws', () => {
  it.each(['antes', 'depois'])('permite membro sem liderança e confirma pelo evento %s da resposta', async order => {
    render(<App />);
    const lobby = { ...testLobby, isLeader: false, members: [
      { ...testLobby.members[0], puuid: 'outro', summonerId: 2, firstPosition: 'top' as const },
      { ...testLobby.members[0], isLeader: false },
    ] };
    hello({ lobby, gameflowPhase: 'Lobby' });
    expect(button('Salvar cargos')).toBeDisabled();
    choosePositions('middle', 'jungle');
    act(() => { fireEvent.click(button('Salvar cargos')); fireEvent.click(button('Salvar cargos')); });
    expect(request().command).toEqual({ type: 'setPositions', first: 'middle', second: 'jungle' });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(positionSelect('Cargo primário')).toBeDisabled();
    expect(button('Sair do lobby')).toBeDisabled();
    expect(panel().getByRole('status')).toHaveTextContent('Enviando cargos');
    expect(roles('Victor#BR1')).toHaveTextContent(/Não selecionado Primário.*Não selecionado Secundário/);
    const update = () => act(() => socket().receive({ type: 'lobby', lobby: { ...lobby, members: [
      lobby.members[0], { ...lobby.members[1], firstPosition: 'middle', secondPosition: 'jungle' },
    ] } }));
    if (order === 'antes') update();
    await ok();
    expect(button('Salvar cargos')).toBeDisabled();
    fireEvent.click(button('Salvar cargos'));
    expect(socket().send).toHaveBeenCalledTimes(1);
    if (order === 'depois') update();
    expect(roles('Victor#BR1')).toHaveTextContent(/Meio Primário.*Selva Secundário/);
    expect(roles('Invocador 2')).toHaveTextContent(/Topo Primário.*Não selecionado Secundário/);
    expect(positionSelect('Cargo primário')).toHaveValue('middle');
  });

  it('normaliza preenchimento e vazio, bloqueia repetidos e permite preenchimento secundário', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('middle', 'jungle');
    expect(within(positionSelect('Cargo secundário')).getByRole('option', { name: 'Meio' })).toBeDisabled();
    fireEvent.change(positionSelect('Cargo primário'), { target: { value: 'jungle' } });
    expect(positionSelect('Cargo secundário')).toHaveValue('unselected');
    choosePositions('middle', 'fill');
    fireEvent.click(button('Salvar cargos'));
    expect(request().command).toEqual({ type: 'setPositions', first: 'middle', second: 'fill' });
    await ok();
    fireEvent.change(positionSelect('Cargo primário'), { target: { value: 'fill' } });
    expect(positionSelect('Cargo secundário')).toBeDisabled();
    expect(positionSelect('Cargo secundário')).toHaveValue('unselected');
    fireEvent.click(button('Salvar cargos'));
    expect(request().command).toEqual({ type: 'setPositions', first: 'fill', second: 'unselected' });
    await ok();
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, members: [
      { ...testLobby.members[0], firstPosition: 'fill' },
    ] } }));
    fireEvent.change(positionSelect('Cargo primário'), { target: { value: 'unselected' } });
    expect(positionSelect('Cargo secundário')).toBeDisabled();
    fireEvent.click(button('Salvar cargos'));
    expect(request().command).toEqual({ type: 'setPositions', first: 'unselected', second: 'unselected' });
    await ok();
  });

  it.each(['aram', 'swiftplay', null] as const)('respeita showPositionSelector na fila %s', queue => {
    render(<App />);
    hello({ lobby: { ...testLobby, queue, showPositionSelector: false }, gameflowPhase: 'Lobby' });
    expect(panel().queryByRole('button', { name: 'Salvar cargos' })).not.toBeInTheDocument();
    expect(panel().getByText('Esta fila não permite escolher cargos.')).toBeInTheDocument();
    expect(socket().send).not.toHaveBeenCalled();
  });

  it('aguarda puuid local, bloqueia outras fases e acompanha mudanças externas', () => {
    render(<App />);
    hello({ lobby: { ...testLobby, members: [{ ...testLobby.members[0], puuid: 'outro' }] }, gameflowPhase: 'Lobby' });
    expect(positionSelect('Cargo primário')).toBeDisabled();
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking' });
    expect(positionSelect('Cargo primário')).toBeDisabled();
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
    choosePositions('middle', 'jungle');
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, members: [
      { ...testLobby.members[0], firstPosition: 'bottom', secondPosition: 'utility' },
    ] } }));
    expect(positionSelect('Cargo primário')).toHaveValue('bottom');
    expect(positionSelect('Cargo secundário')).toHaveValue('utility');
    expect(button('Salvar cargos')).toBeDisabled();
    expect(socket().send).not.toHaveBeenCalled();
  });

  it.each(['lcu', 'invalidCommand', 'notConnected'] as const)('trata erro %s e permite nova tentativa explícita', async code => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    fireEvent.click(button('Salvar cargos'));
    await act(async () => socket().receive({ type: 'response', id: request().id,
      result: { status: 'error', code, message: 'Cargos indisponíveis' } }));
    expect(panel().getByRole('alert')).toHaveTextContent(code === 'notConnected' ? 'desconectou' : 'Cargos indisponíveis');
    expect(roles('Victor#BR1')).toHaveTextContent(/Não selecionado Primário.*Não selecionado Secundário/);
    expect(button('Salvar cargos')).toBeEnabled();
    fireEvent.click(button('Salvar cargos'));
    await ok();
  });

  it('trata falha de envio, timeout e resposta tardia sem repetir automaticamente', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    socket().send.mockImplementationOnce(() => { throw new Error('envio'); });
    await act(async () => fireEvent.click(button('Salvar cargos')));
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível confirmar');
    fireEvent.click(button('Salvar cargos'));
    const id = request().id;
    await act(async () => vi.advanceTimersByTime(10000));
    expect(button('Salvar cargos')).toBeEnabled();
    await act(async () => socket().receive({ type: 'response', id, result: { status: 'ok' } }));
    expect(panel().getByRole('alert')).toBeInTheDocument();
    expect(socket().send).toHaveBeenCalledTimes(2);
  });

  it.each(['agente', 'LoL', 'sessão', 'fila'] as const)('descarta confirmação anterior após mudança de %s', async source => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    fireEvent.click(button('Salvar cargos'));
    const old = socket();
    const id = request().id;
    if (source === 'agente') {
      await act(async () => old.onclose?.());
      expect(panel().queryByRole('button')).not.toBeInTheDocument();
      act(() => vi.advanceTimersByTime(1000));
      hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    } else if (source === 'fila') {
      act(() => old.receive({ type: 'lobby', lobby: { ...testLobby, queue: 'aram', queueId: 450, showPositionSelector: false } }));
    } else {
      act(() => old.receive({ type: 'lcu', status: source === 'LoL'
        ? { state: 'disconnected', reason: 'fechado' } : { state: 'connected', session: 2 } }));
      if (source === 'LoL') hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    }
    await act(async () => old.receive({ type: 'response', id, result: { status: 'ok' } }));
    expect(panel().queryByText(/Comando de cargos aceito/)).not.toBeInTheDocument();
    expect(socket().send).toHaveBeenCalledTimes(source === 'agente' ? 0 : 1);
  });
});

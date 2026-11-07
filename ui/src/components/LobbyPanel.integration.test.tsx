import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import { AGENT_URL } from '../agent/client';
import { AUTO_SAVE_DELAY_MS } from './LobbyPanel';
import { queueNames } from '../agent/queues';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { GameflowPhase } from '../bindings/GameflowPhase';
import type { GameQueue } from '../bindings/GameQueue';
import type { Queue } from '../bindings/Queue';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testFriend } from '../test/events';
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
/** Espera o atraso da gravação automática (e o envio seguinte, se houver um na espera). */
async function settle(ms = AUTO_SAVE_DELAY_MS) {
  await act(async () => { vi.advanceTimersByTime(ms); });
}
// Estes testes verificam também as guardas dos painéis montados fora da tela ativa.
beforeEach(() => { configure({ defaultHidden: true }); vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.useRealTimers(); vi.unstubAllGlobals(); });

const queueSelect = () => panel().getByRole('combobox', { name: 'Fila' });
const gameQueue = (queueId: number, name: string, gameMode: string, versusAi = false): GameQueue => ({
  queueId, queue: null, name, description: name, gameMode, modeGroup: gameMode === 'KIWI' ? 'kARAM' : 'kOther', mapId: 12,
  versusAi, ranked: false, showPositionSelector: false, limitedTime: false,
});
const desordem = gameQueue(2400, 'ARAM: Desordem', 'KIWI');
const jade = gameQueue(4310, 'Jade', 'JADE');
const coop = gameQueue(870, 'Introdução', 'CLASSIC', true);

describe('lobby pelo único /ws do App', () => {
  it.each(Object.keys(queueNames) as Queue[])('cria %s sem duplicar e aguarda estado e resposta do agente', async queue => {
    render(<App />);
    expect(Socket.instances).toHaveLength(1);
    expect(socket().url).toBe(AGENT_URL);
    hello();
    expect(panel().getAllByRole('option')).toHaveLength(5);
    choose(queue);
    await settle();
    // Sem lobby, escolher a fila não cria nada sozinho.
    expect(socket().send).not.toHaveBeenCalled();
    act(() => { fireEvent.click(button('Criar lobby')); fireEvent.click(button('Criar lobby')); });
    expect(request().command).toEqual({ type: 'createLobby', queue });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(panel().queryByRole('button', { name: 'Sair do lobby' })).not.toBeInTheDocument();
    expect(queueSelect()).toBeDisabled();
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
    expect(panel().queryByRole('button', { name: 'Trocar fila' })).not.toBeInTheDocument();
    expect(queueSelect()).toHaveValue(queue);
    expect(queueSelect()).toBeEnabled();
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

  it('recebe lobby no hello, troca fila ao escolher, segue eventos externos e sai sem estado otimista', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    expect(panel().getByRole('heading', { name: 'Victor#BR1' })).toBeInTheDocument();
    expect(panel().getByText('Nível 122')).toBeInTheDocument();
    expect(panel().getByRole('img', { name: 'Líder do grupo' })).toBeInTheDocument();
    expect(panel().getByText(/1 \/ 2 jogadores/)).toBeInTheDocument();
    expect(queueSelect()).toHaveValue('rankedSolo');
    choose('aram');
    expect(socket().send).not.toHaveBeenCalled();
    await settle();
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
    expect(queueSelect()).toHaveValue('aram');
    expect(panel().getByText(queueNames.rankedSolo, { selector: 'strong' })).toBeInTheDocument();
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: 'aram', queueId: 450, maxLobbySize: 5 } }));
    await ok();
    expect(queueSelect()).toHaveValue('aram');
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: null, queueId: 999 } }));
    expect(panel().getByText('Outra fila (ID 999)', { selector: 'strong' })).toBeInTheDocument();
    fireEvent.click(button('Sair do lobby'));
    fireEvent.click(button('Sair do lobby'));
    expect(socket().send).toHaveBeenCalledTimes(2);
    expect(request().command).toEqual({ type: 'leaveLobby' });
    expect(panel().getByText('Outra fila (ID 999)', { selector: 'strong' })).toBeInTheDocument();
    act(() => {
      socket().receive({ type: 'lobby', lobby: null });
      socket().receive({ type: 'gameflowPhase', phase: 'None' });
    });
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('Saída do lobby confirmada');
    expect(panel().getByText('Você não está em um lobby.')).toBeInTheDocument();
    expect(button('Criar lobby')).toBeEnabled();
  });

  it('envia só a última fila de uma troca rápida, inclusive escolhida durante o envio', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choose('aram');
    choose('rankedFlex');
    choose('normalDraft');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(request().command).toEqual({ type: 'createLobby', queue: 'normalDraft' });
    // Durante o envio, o seletor continua livre; a nova escolha espera a resposta.
    expect(queueSelect()).toBeEnabled();
    choose('aram');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: 'normalDraft', queueId: 400, maxLobbySize: 5 } }));
    // O evento da fila anterior não apaga a escolha que ainda vai sair.
    expect(queueSelect()).toHaveValue('aram');
    await ok();
    await settle(0);
    expect(socket().send).toHaveBeenCalledTimes(2);
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: 'aram', queueId: 450, maxLobbySize: 5 } }));
    await ok();
    expect(queueSelect()).toHaveValue('aram');
  });

  it('voltar para a fila atual antes do envio cancela a troca', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choose('aram');
    choose('rankedSolo');
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
    expect(queueSelect()).toHaveValue('rankedSolo');
  });

  it('impede troca por membro, mas permite sair; reflete mudança de liderança', async () => {
    render(<App />);
    hello({ gameflowPhase: 'Lobby', lobby: { ...testLobby, isLeader: false } });
    expect(queueSelect()).toBeDisabled();
    expect(panel().getByText(/Somente o líder/)).toBeInTheDocument();
    choose('aram');
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
    expect(button('Sair do lobby')).toBeEnabled();
    act(() => socket().receive({ type: 'lobby', lobby: testLobby }));
    expect(queueSelect()).toBeEnabled();
    choose('aram');
    await settle();
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
  });

  it('descarta a troca agendada se a liderança mudar antes do envio', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choose('aram');
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, isLeader: false } }));
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
    expect(queueSelect()).toHaveValue('rankedSolo');
  });

  const blockedPhases: (GameflowPhase | null)[] = [null, 'Unknown', 'Matchmaking', 'ReadyCheck', 'ChampSelect',
    'GameStart', 'InProgress', 'Reconnect', 'EndOfGame', 'CheckedIntoTournament', 'FailedToLaunch',
    'WaitingForStats', 'PreEndOfGame', 'TerminatedInError'];
  it.each(blockedPhases)('bloqueia criar, trocar e sair na fase %s', async phase => {
    render(<App />);
    hello({ gameflowPhase: phase });
    expect(button('Criar lobby')).toBeDisabled();
    fireEvent.click(button('Criar lobby'));
    hello({ gameflowPhase: phase, lobby: testLobby });
    expect(queueSelect()).toBeDisabled();
    choose('aram');
    await settle();
    expect(button('Sair do lobby')).toBeDisabled();
    expect(socket().send).not.toHaveBeenCalled();
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
    expect(button('Sair do lobby')).toBeEnabled();
  });

  it('não confunde canStartActivity com permissão para trocar fila e aguarda login', async () => {
    render(<App />);
    hello({ summoner: null });
    expect(button('Criar lobby')).toBeDisabled();
    expect(panel().getByText(/Aguarde o login/)).toBeInTheDocument();
    hello({ lobby: { ...testLobby, canStartActivity: false }, gameflowPhase: 'Lobby' });
    expect(queueSelect()).toBeEnabled();
    choose('aram');
    await settle();
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
    expect(button('Sair do lobby')).toBeDisabled();
  });

  it.each(['notConnected', 'lcu', 'invalidCommand', 'launchFailed'] as const)('exibe erro %s, volta o seletor e permite escolher de novo', async code => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choose('aram');
    await settle();
    await act(async () => socket().receive({ type: 'response', id: request().id, result: { status: 'error', code, message: 'Fila indisponível' } }));
    expect(panel().getByRole('alert')).toHaveTextContent(code === 'notConnected' ? 'desconectou' : 'Fila indisponível');
    expect(panel().getByText(queueNames.rankedSolo, { selector: 'strong' })).toBeInTheDocument();
    expect(queueSelect()).toHaveValue('rankedSolo');
    expect(socket().close).not.toHaveBeenCalled();
    expect(Socket.instances).toHaveLength(1);
    expect(socket().send).toHaveBeenCalledTimes(1);
    choose('aram');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(2);
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

  it('oferece a ARAM: Desordem e as outras filas da LCU, cria pelo id e troca de volta para uma fixa', async () => {
    render(<App />);
    hello({ queues: [desordem, jade, coop] });
    expect(panel().getAllByRole('option')).toHaveLength(8);
    expect(panel().getByRole('group', { name: 'Mais modos' })).toHaveTextContent('ARAM: DesordemJade');
    expect(panel().getByRole('group', { name: 'Contra IA' })).toHaveTextContent('Introdução');
    fireEvent.change(queueSelect(), { target: { value: 'fila-2400' } });
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
    fireEvent.click(button('Criar lobby'));
    expect(request().command).toEqual({ type: 'createLobbyById', queueId: 2400 });
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: null, queueId: 2400, maxLobbySize: 5, showPositionSelector: false } }));
    await ok();
    expect(panel().getByText('ARAM: Desordem', { selector: 'strong' })).toBeInTheDocument();
    expect(queueSelect()).toHaveValue('fila-2400');
    // Escolher a mesma fila não manda nada; outra fila troca sozinha.
    fireEvent.change(queueSelect(), { target: { value: 'aram' } });
    await settle();
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
  });

  it('mostra a fila atual mesmo quando ela não está na lista', () => {
    render(<App />);
    hello({ lobby: { ...testLobby, queue: null, queueId: 999 }, gameflowPhase: 'Lobby', queues: [desordem] });
    expect(queueSelect()).toHaveValue('fila-999');
    expect(panel().getByRole('option', { name: 'Outra fila (ID 999)' })).toBeDisabled();
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

  it('não envia troca agendada depois que o LoL desconecta', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choose('aram');
    act(() => socket().receive({ type: 'lcu', status: { state: 'disconnected', reason: 'fechado' } }));
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
  });
});

const positionSelect = (name: string) => panel().getByRole('combobox', { name });
// Cargos confirmados de um jogador, como aparecem nas etiquetas da linha dele.
const roles = (name: string) => panel().getByRole('list', { name: `Posições de ${name}` });
function choosePositions(first: string, second = 'unselected') {
  fireEvent.change(positionSelect('Posição primária'), { target: { value: first } });
  fireEvent.change(positionSelect('Posição secundária'), { target: { value: second } });
}

describe('cargos pelo /ws', () => {
  it.each(['antes', 'depois'])('salva sozinho para membro sem liderança e confirma pelo evento %s da resposta', async order => {
    render(<App />);
    const lobby = { ...testLobby, isLeader: false, members: [
      { ...testLobby.members[0], puuid: 'outro', summonerId: 2, firstPosition: 'top' as const },
      { ...testLobby.members[0], isLeader: false },
    ] };
    hello({ lobby, gameflowPhase: 'Lobby' });
    expect(panel().queryByRole('button', { name: 'Salvar posições' })).not.toBeInTheDocument();
    fireEvent.change(positionSelect('Posição primária'), { target: { value: 'middle' } });
    await settle();
    // Só o primário: espera o secundário.
    expect(socket().send).not.toHaveBeenCalled();
    expect(panel().getByText('Escolha a posição secundária para salvar.')).toBeInTheDocument();
    fireEvent.change(positionSelect('Posição secundária'), { target: { value: 'jungle' } });
    expect(socket().send).not.toHaveBeenCalled();
    await settle();
    expect(request().command).toEqual({ type: 'setPositions', first: 'middle', second: 'jungle' });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(positionSelect('Posição primária')).toBeEnabled();
    expect(button('Sair do lobby')).toBeDisabled();
    expect(panel().getByRole('status')).toHaveTextContent('Salvando posições');
    expect(roles('Victor#BR1')).toHaveTextContent(/Não selecionado Primária.*Não selecionado Secundária/);
    const update = () => act(() => socket().receive({ type: 'lobby', lobby: { ...lobby, members: [
      lobby.members[0], { ...lobby.members[1], firstPosition: 'middle', secondPosition: 'jungle' },
    ] } }));
    if (order === 'antes') update();
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('Posições salvas.');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    if (order === 'depois') update();
    expect(roles('Victor#BR1')).toHaveTextContent(/Meio Primária.*Selva Secundária/);
    expect(roles('Victor#BR1').querySelectorAll('svg[aria-hidden="true"]')).toHaveLength(2);
    expect(roles('Jogador')).toHaveTextContent(/Topo Primária.*Não selecionado Secundária/);
    expect(positionSelect('Posição primária')).toHaveValue('middle');
    expect(positionSelect('Posição secundária')).toHaveValue('jungle');
  });

  it('mostra o Riot ID dos amigos no lobby e "Jogador" para quem não é amigo', () => {
    render(<App />);
    const member = { ...testLobby.members[0], isLeader: false };
    hello({ gameflowPhase: 'Lobby', friends: [{ ...testFriend, puuid: 'ana', gameName: 'Games e Musica', tagLine: 'vsfd' }],
      lobby: { ...testLobby, maxLobbySize: 5, members: [testLobby.members[0],
        { ...member, puuid: 'ana', summonerId: 172 }, { ...member, puuid: 'outro', summonerId: 173 }] } });
    const players = within(panel().getByRole('list', { name: 'Jogadores no lobby' }));
    expect(players.getByRole('heading', { name: 'Games e Musica#vsfd' })).toBeInTheDocument();
    expect(players.getByRole('heading', { name: 'Jogador' })).toBeInTheDocument();
    expect(players.queryByText(/Invocador/)).not.toBeInTheDocument();
  });

  it('normaliza preenchimento e vazio, bloqueia repetidos e permite preenchimento secundário', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('middle', 'jungle');
    expect(within(positionSelect('Posição secundária')).getByRole('option', { name: 'Meio' })).toBeDisabled();
    fireEvent.change(positionSelect('Posição primária'), { target: { value: 'jungle' } });
    expect(positionSelect('Posição secundária')).toHaveValue('unselected');
    choosePositions('middle', 'fill');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(request().command).toEqual({ type: 'setPositions', first: 'middle', second: 'fill' });
    await ok();
    fireEvent.change(positionSelect('Posição primária'), { target: { value: 'fill' } });
    expect(positionSelect('Posição secundária')).toBeDisabled();
    expect(positionSelect('Posição secundária')).toHaveValue('unselected');
    await settle();
    expect(request().command).toEqual({ type: 'setPositions', first: 'fill', second: 'unselected' });
    await ok();
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, members: [
      { ...testLobby.members[0], firstPosition: 'fill' },
    ] } }));
    fireEvent.change(positionSelect('Posição primária'), { target: { value: 'unselected' } });
    expect(positionSelect('Posição secundária')).toBeDisabled();
    await settle();
    expect(request().command).toEqual({ type: 'setPositions', first: 'unselected', second: 'unselected' });
    await ok();
    expect(socket().send).toHaveBeenCalledTimes(3);
  });

  it('troca rápida termina no último par escolhido, mesmo durante um envio', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    choosePositions('middle', 'bottom');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(request().command).toEqual({ type: 'setPositions', first: 'middle', second: 'bottom' });
    choosePositions('utility', 'top');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, members: [
      { ...testLobby.members[0], firstPosition: 'middle', secondPosition: 'bottom' },
    ] } }));
    // O evento do par anterior não apaga a escolha que ainda vai sair.
    expect(positionSelect('Posição primária')).toHaveValue('utility');
    expect(positionSelect('Posição secundária')).toHaveValue('top');
    await ok();
    await settle(0);
    expect(socket().send).toHaveBeenCalledTimes(2);
    expect(request().command).toEqual({ type: 'setPositions', first: 'utility', second: 'top' });
    await ok();
  });

  it('voltar ao par aceito antes do evento não envia de novo', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    await settle();
    await ok();
    choosePositions('middle', 'bottom');
    choosePositions('top', 'jungle');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(positionSelect('Posição primária')).toHaveValue('top');
  });

  it.each(['aram', 'swiftplay', null] as const)('respeita showPositionSelector na fila %s', queue => {
    render(<App />);
    hello({ lobby: { ...testLobby, queue, showPositionSelector: false }, gameflowPhase: 'Lobby' });
    expect(panel().queryByRole('combobox', { name: 'Posição primária' })).not.toBeInTheDocument();
    expect(panel().getByText('Esta fila não permite escolher posições.')).toBeInTheDocument();
    expect(socket().send).not.toHaveBeenCalled();
  });

  it('aguarda puuid local, bloqueia outras fases e acompanha mudanças externas', async () => {
    render(<App />);
    hello({ lobby: { ...testLobby, members: [{ ...testLobby.members[0], puuid: 'outro' }] }, gameflowPhase: 'Lobby' });
    expect(positionSelect('Posição primária')).toBeDisabled();
    hello({ lobby: testLobby, gameflowPhase: 'Matchmaking' });
    expect(positionSelect('Posição primária')).toBeDisabled();
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
    fireEvent.change(positionSelect('Posição primária'), { target: { value: 'middle' } });
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, members: [
      { ...testLobby.members[0], firstPosition: 'bottom', secondPosition: 'utility' },
    ] } }));
    expect(positionSelect('Posição primária')).toHaveValue('bottom');
    expect(positionSelect('Posição secundária')).toHaveValue('utility');
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
  });

  it('descarta cargos agendados quando a busca começa antes do envio', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Matchmaking' }));
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
  });

  it.each(['lcu', 'invalidCommand', 'notConnected'] as const)('trata erro %s, volta aos cargos confirmados e permite escolher de novo', async code => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    await settle();
    await act(async () => socket().receive({ type: 'response', id: request().id,
      result: { status: 'error', code, message: 'Posições indisponíveis' } }));
    expect(panel().getByRole('alert')).toHaveTextContent(code === 'notConnected' ? 'desconectou' : 'Posições indisponíveis');
    expect(roles('Victor#BR1')).toHaveTextContent(/Não selecionado Primária.*Não selecionado Secundária/);
    expect(positionSelect('Posição primária')).toHaveValue('unselected');
    expect(positionSelect('Posição secundária')).toHaveValue('unselected');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(1);
    choosePositions('top', 'jungle');
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(2);
    await ok();
  });

  it('trata falha de envio, timeout e resposta tardia sem repetir automaticamente', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    socket().send.mockImplementationOnce(() => { throw new Error('envio'); });
    await act(async () => { vi.advanceTimersByTime(AUTO_SAVE_DELAY_MS); });
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível confirmar');
    expect(positionSelect('Posição primária')).toHaveValue('unselected');
    choosePositions('top', 'jungle');
    await settle();
    const id = request().id;
    await act(async () => vi.advanceTimersByTime(10000));
    expect(panel().getByRole('alert')).toBeInTheDocument();
    await act(async () => socket().receive({ type: 'response', id, result: { status: 'ok' } }));
    expect(panel().getByRole('alert')).toBeInTheDocument();
    expect(socket().send).toHaveBeenCalledTimes(2);
  });

  it.each(['agente', 'LoL', 'sessão', 'fila'] as const)('descarta confirmação anterior após mudança de %s', async source => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    choosePositions('top', 'jungle');
    await settle();
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
    expect(panel().queryByText('Posições salvas.')).not.toBeInTheDocument();
    await settle();
    expect(socket().send).toHaveBeenCalledTimes(source === 'agente' ? 0 : 1);
  });
});

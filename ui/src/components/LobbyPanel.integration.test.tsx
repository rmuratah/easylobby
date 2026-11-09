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
import type { Matchmaking } from '../bindings/Matchmaking';
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
/** No lobby, troca a fila pela tela de escolha de modo (#231). */
function changeQueue(submode: string) {
  fireEvent.click(button('Trocar modo'));
  fireEvent.click(radio(submode));
  fireEvent.click(button('Trocar fila'));
}
/** Espera o atraso da gravação automática (e o envio seguinte, se houver um na espera). */
async function settle(ms = AUTO_SAVE_DELAY_MS) {
  await act(async () => { vi.advanceTimersByTime(ms); });
}
// Estes testes verificam também as guardas dos painéis montados fora da tela ativa.
beforeEach(() => { configure({ defaultHidden: true }); vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.useRealTimers(); vi.unstubAllGlobals(); });

/** Nomes dos submodos na tela de escolha de modo (#210). */
const modeNames: Record<Queue, string> = {
  swiftplay: 'Partida Dinâmica', normalDraft: 'Partida Alternada', rankedFlex: 'Ranqueada Flex', rankedSolo: 'Ranqueada Solo/Duo', aram: 'ARAM Normal',
};
const radio = (name: string) => panel().getByRole('radio', { name: new RegExp(`^${name}`) });
const gameQueue = (queueId: number, name: string, gameMode: string, versusAi = false): GameQueue => ({
  queueId, queue: null, name, description: name, gameMode, modeGroup: gameMode === 'KIWI' ? 'kARAM' : 'kOther', mapId: 12,
  versusAi, ranked: false, showPositionSelector: false, limitedTime: false, available: true,
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
    expect(panel().getByRole('tab', { name: 'PvP' })).toHaveAttribute('aria-selected', 'true');
    fireEvent.click(radio(modeNames[queue]));
    expect(radio(modeNames[queue])).toBeChecked();
    await settle();
    // Sem lobby, escolher a fila não cria nada sozinho.
    expect(socket().send).not.toHaveBeenCalled();
    act(() => { fireEvent.click(button('Criar lobby')); fireEvent.click(button('Criar lobby')); });
    expect(request().command).toEqual({ type: 'createLobby', queue });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(panel().queryByRole('button', { name: 'Sair do lobby' })).not.toBeInTheDocument();
    expect(radio(modeNames[queue])).toBeDisabled();
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
    expect(panel().queryByRole('combobox', { name: 'Fila' })).not.toBeInTheDocument();
    expect(button('Trocar modo')).toBeEnabled();
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

  it('recebe lobby no hello, troca a fila pela tela de modos, segue eventos externos e sai sem estado otimista', async () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    expect(panel().getByRole('heading', { name: 'Victor#BR1' })).toBeInTheDocument();
    expect(panel().getByText('Nível 122')).toBeInTheDocument();
    expect(panel().getByRole('img', { name: 'Líder do grupo' })).toBeInTheDocument();
    expect(panel().getByText(/1 \/ 2 jogadores/)).toBeInTheDocument();
    fireEvent.click(button('Trocar modo'));
    // A tela abre na aba da fila atual, sem a aba Treinamento; a fila atual fica bloqueada (#239)
    // e a seleção começa em outra fila aberta do mesmo modo.
    expect(panel().getByText('Você está no lobby de Ranqueada Solo/Duo.')).toBeInTheDocument();
    expect(radio('Ranqueada Solo/Duo')).toBeDisabled();
    expect(radio('Ranqueada Solo/Duo')).not.toBeChecked();
    expect(radio('Ranqueada Solo/Duo').closest('label')).toHaveClass('cursor-not-allowed');
    expect(radio('Ranqueada Solo/Duo').closest('label')).toHaveTextContent('Você já está nesta fila.');
    expect(radio('Partida Dinâmica')).toBeChecked();
    expect(button('Trocar fila')).toBeEnabled();
    expect(panel().queryByRole('tab', { name: 'Treinamento' })).not.toBeInTheDocument();
    fireEvent.click(radio('ARAM Normal'));
    expect(socket().send).not.toHaveBeenCalled();
    act(() => { fireEvent.click(button('Trocar fila')); fireEvent.click(button('Trocar fila')); });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
    expect(panel().getByRole('status')).toHaveTextContent('Trocando fila');
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: 'aram', queueId: 450, maxLobbySize: 5 } }));
    await ok();
    // Deu certo: volta ao lobby, já na fila nova.
    expect(panel().getByText(queueNames.aram, { selector: 'strong' })).toBeInTheDocument();
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

  it('voltar ao lobby fecha a tela de modos sem trocar a fila', () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    fireEvent.click(button('Trocar modo'));
    fireEvent.click(radio('ARAM Normal'));
    fireEvent.click(button('Voltar ao lobby'));
    expect(socket().send).not.toHaveBeenCalled();
    expect(panel().getByText(queueNames.rankedSolo, { selector: 'strong' })).toBeInTheDocument();
    // Reabrir começa de novo pela aba da fila atual.
    fireEvent.click(button('Trocar modo'));
    expect(radio('Partida Dinâmica')).toBeChecked();
    expect(radio('ARAM Normal')).not.toBeChecked();
  });

  it('volta ao lobby quando a busca começa com a tela de modos aberta', () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    fireEvent.click(button('Trocar modo'));
    act(() => socket().receive({ type: 'matchmaking', matchmaking: { searchState: 'Searching', queueId: 420, timeInQueue: 0, estimatedQueueTime: 0, lowPriorityTimeRemaining: 0, errors: [] } }));
    expect(panel().getByText(queueNames.rankedSolo, { selector: 'strong' })).toBeInTheDocument();
    expect(panel().queryByRole('button', { name: 'Trocar modo' })).not.toBeInTheDocument();
  });

  it('impede troca por membro, mas permite sair; reflete mudança de liderança', async () => {
    render(<App />);
    hello({ gameflowPhase: 'Lobby', lobby: { ...testLobby, isLeader: false } });
    expect(button('Trocar modo')).toBeDisabled();
    expect(panel().getByText(/Somente o líder/)).toBeInTheDocument();
    expect(button('Sair do lobby')).toBeEnabled();
    act(() => socket().receive({ type: 'lobby', lobby: testLobby }));
    changeQueue('ARAM Normal');
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
  });

  it('não troca a fila se a liderança mudar com a tela de modos aberta', () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    fireEvent.click(button('Trocar modo'));
    fireEvent.click(radio('ARAM Normal'));
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, isLeader: false } }));
    expect(button('Trocar fila')).toBeDisabled();
    fireEvent.click(button('Trocar fila'));
    expect(socket().send).not.toHaveBeenCalled();
    expect(panel().getByText(/Somente o líder/)).toBeInTheDocument();
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
    expect(button('Trocar modo')).toBeDisabled();
    expect(button('Sair do lobby')).toBeDisabled();
    expect(socket().send).not.toHaveBeenCalled();
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
    expect(button('Sair do lobby')).toBeEnabled();
  });

  const dodge: Matchmaking = {
    searchState: 'Error', queueId: 420, timeInQueue: 0, estimatedQueueTime: 0, lowPriorityTimeRemaining: 0,
    errors: [{ errorType: 'QUEUE_DODGER', message: '', penaltyTimeRemaining: 300 }],
  };
  it.each(['Matchmaking', 'Lobby'] as const)('deixa sair do lobby com a busca parada por punição na fase %s', async phase => {
    render(<App />);
    hello({ gameflowPhase: phase, lobby: testLobby, matchmaking: dodge });
    fireEvent.click(button('Sair do lobby'));
    expect(request().command).toEqual({ type: 'leaveLobby' });
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('Saída do lobby confirmada.');
  });

  it('esconde sair do lobby na busca sem punição e na partida encontrada', () => {
    render(<App />);
    hello({ gameflowPhase: 'Matchmaking', lobby: testLobby, matchmaking: { ...dodge, searchState: 'Searching', errors: [] } });
    expect(panel().queryByRole('button', { name: 'Sair do lobby' })).not.toBeInTheDocument();
    hello({ gameflowPhase: 'ReadyCheck', lobby: testLobby, matchmaking: { ...dodge, searchState: 'Found' } });
    expect(panel().queryByRole('button', { name: 'Sair do lobby' })).not.toBeInTheDocument();
  });

  it('não confunde canStartActivity com permissão para trocar fila e aguarda login', async () => {
    render(<App />);
    hello({ summoner: null });
    expect(button('Criar lobby')).toBeDisabled();
    expect(panel().getByText(/Aguarde o login/)).toBeInTheDocument();
    hello({ lobby: { ...testLobby, canStartActivity: false }, gameflowPhase: 'Lobby' });
    changeQueue('ARAM Normal');
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
    await ok();
    expect(button('Sair do lobby')).toBeEnabled();
  });

  it.each(['notConnected', 'lcu', 'invalidCommand', 'launchFailed'] as const)('exibe erro %s, mantém a tela de modos e permite tentar de novo', async code => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    changeQueue('ARAM Normal');
    await act(async () => socket().receive({ type: 'response', id: request().id, result: { status: 'error', code, message: 'Fila indisponível' } }));
    expect(panel().getByRole('alert')).toHaveTextContent(code === 'notConnected' ? 'desconectou' : 'Fila indisponível');
    expect(panel().getByText('Você está no lobby de Ranqueada Solo/Duo.')).toBeInTheDocument();
    expect(radio('ARAM Normal')).toBeChecked();
    expect(socket().close).not.toHaveBeenCalled();
    expect(Socket.instances).toHaveLength(1);
    expect(socket().send).toHaveBeenCalledTimes(1);
    fireEvent.click(button('Trocar fila'));
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
    // A Desordem vai para o card ARAM; a Jade, para o Summoner's Rift Clássico.
    expect(panel().getByRole('group', { name: "Summoner's Rift Clássico" })).toHaveTextContent('Jade');
    fireEvent.click(radio('ARAM Desordem'));
    await settle();
    expect(socket().send).not.toHaveBeenCalled();
    fireEvent.click(button('Criar lobby'));
    expect(request().command).toEqual({ type: 'createLobbyById', queueId: 2400 });
    act(() => socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: null, queueId: 2400, maxLobbySize: 5, showPositionSelector: false } }));
    await ok();
    expect(panel().getByText('ARAM: Desordem', { selector: 'strong' })).toBeInTheDocument();
    // A Desordem fica bloqueada na tela de modos (#239), que já abre no ARAM Normal.
    fireEvent.click(button('Trocar modo'));
    expect(radio('ARAM Desordem')).toBeDisabled();
    expect(radio('ARAM Desordem')).not.toBeChecked();
    expect(radio('ARAM Normal')).toBeChecked();
    fireEvent.click(button('Trocar fila'));
    expect(request().command).toEqual({ type: 'createLobby', queue: 'aram' });
  });

  it('troca de aba, inicia o Tutorial e o Modo de Treino pelo botão de confirmar', async () => {
    render(<App />);
    hello({ queues: [coop] });
    fireEvent.click(panel().getByRole('tab', { name: 'Co-op versus IA' }));
    expect(radio('Introdução')).toBeChecked();
    fireEvent.click(button('Criar lobby'));
    expect(request().command).toEqual({ type: 'createLobbyById', queueId: 870 });
    await ok();
    fireEvent.click(panel().getByRole('tab', { name: 'Treinamento' }));
    expect(radio('Tutorial')).toBeChecked();
    fireEvent.click(button('Iniciar'));
    expect(request().command).toEqual({ type: 'startTutorial', queueId: 2000 });
    expect(panel().getByRole('status')).toHaveTextContent('Iniciando o Tutorial');
    await ok();
    fireEvent.click(radio('Modo de Treino'));
    fireEvent.click(button('Iniciar'));
    expect(request().command).toEqual({ type: 'startPracticeTool' });
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('Modo de Treino iniciado');
  });

  it('cria a partida personalizada pelo formulário da aba Personalizada (#262, #270)', async () => {
    render(<App />);
    hello();
    fireEvent.click(panel().getByRole('tab', { name: 'Personalizada' }));
    const name = panel().getByRole('textbox', { name: 'Nome' });
    expect(name).toHaveValue('Partida de Victor');
    expect(panel().getByText("Summoner's Rift · 5v5 · Escolha às Cegas")).toBeInTheDocument();
    // Padrões do client oficial: espectadores Todos e com intervalo.
    expect(panel().getByRole('radio', { name: 'Todos' })).toBeChecked();
    expect(panel().getByRole('radio', { name: 'Adicionar intervalo' })).toBeChecked();
    const pick = panel().getByRole('combobox', { name: 'Tipo de jogo' });
    expect(within(pick).getAllByRole('option').map(o => o.textContent))
      .toEqual(['Escolha às Cegas', 'Modo alternado', 'Todos aleatórios', 'Alternada Torneio']);
    // Nome vazio não sai.
    fireEvent.change(name, { target: { value: '   ' } });
    expect(button('Confirmar')).toBeDisabled();
    expect(panel().getByText('Dê um nome para a partida.')).toBeInTheDocument();
    fireEvent.change(name, { target: { value: '  Treino da firma ' } });
    fireEvent.change(panel().getByRole('textbox', { name: 'Senha (opcional)' }), { target: { value: 'segredo' } });
    fireEvent.change(pick, { target: { value: 'allRandom' } });
    // Rift Clássico: só alternada e às cegas; o tipo que não existe nele volta para o primeiro.
    fireEvent.click(radio('Rift Clássico'));
    expect(within(pick).getAllByRole('option').map(o => o.textContent)).toEqual(['Escolha Alternada', 'Escolha às Cegas']);
    expect(pick).toHaveValue('draftMode');
    fireEvent.change(panel().getByRole('combobox', { name: 'Tamanho da equipe' }), { target: { value: '3' } });
    fireEvent.click(panel().getByRole('radio', { name: 'Somente da lista de amigos' }));
    fireEvent.click(panel().getByRole('radio', { name: 'Sem intervalo' }));
    fireEvent.click(panel().getByRole('checkbox', { name: 'Esconder da lista de saguões públicos' }));
    expect(panel().getByText('Rift Clássico · 3v3 · Escolha Alternada · Com senha')).toBeInTheDocument();
    fireEvent.click(button('Confirmar'));
    expect(request().command).toEqual({ type: 'createCustomGame', options: {
      name: 'Treino da firma', password: 'segredo', map: 'classicRift', pickType: 'draftMode', aramMap: 'howlingAbyss', teamSize: 3,
      spectators: 'friendsOnly', hideFromPublicList: true, spectatorDelay: false,
    } });
    expect(panel().getByRole('status')).toHaveTextContent('Criando a partida personalizada');
    await ok();
    expect(panel().getByRole('status')).toHaveTextContent('Partida personalizada criada');
  });

  it('o nome sugerido da personalizada usa o invocador que chega depois, sem apagar o que o jogador digitou (#278)', () => {
    render(<App />);
    const victor = testState.summoner!;
    hello({ summoner: { ...victor, gameName: '', displayName: '' } });
    fireEvent.click(panel().getByRole('tab', { name: 'Personalizada' }));
    const name = () => panel().getByRole('textbox', { name: 'Nome' });
    expect(name()).toHaveValue('Partida personalizada');
    act(() => socket().receive({ type: 'summoner', summoner: victor }));
    expect(name()).toHaveValue('Partida de Victor');
    fireEvent.change(name(), { target: { value: 'Treino da firma' } });
    act(() => socket().receive({ type: 'summoner', summoner: { ...victor, gameName: 'Outro' } }));
    expect(name()).toHaveValue('Treino da firma');
  });

  it('cria a personalizada no Mapa aleatório com a seleção de mapas (#271)', async () => {
    render(<App />);
    hello();
    fireEvent.click(panel().getByRole('tab', { name: 'Personalizada' }));
    expect(panel().queryByRole('combobox', { name: 'Seleção de mapas' })).not.toBeInTheDocument();
    fireEvent.click(radio('Mapa aleatório'));
    const pick = panel().getByRole('combobox', { name: 'Tipo de jogo' });
    expect(within(pick).getAllByRole('option').map(o => o.textContent))
      .toEqual(['Todos aleatórios', 'ARAM: Desordem', 'Escolha às Cegas', 'Alternada Torneio', 'Modo alternado']);
    const maps = panel().getByRole('combobox', { name: 'Seleção de mapas' });
    expect(maps).toHaveValue('howlingAbyss');
    expect(within(maps).getAllByRole('option').map(o => o.textContent))
      .toEqual(['Howling Abyss', 'Travessia de Koeshin', 'Ponte da Carnificina', 'Mapa aleatório']);
    fireEvent.change(pick, { target: { value: 'aramMayhem' } });
    fireEvent.change(maps, { target: { value: 'butchersBridge' } });
    expect(panel().getByText('Ponte da Carnificina · 5v5 · ARAM: Desordem')).toBeInTheDocument();
    fireEvent.click(button('Confirmar'));
    expect(request().command).toMatchObject({ type: 'createCustomGame', options: {
      map: 'howlingAbyss', pickType: 'aramMayhem', aramMap: 'butchersBridge',
    } });
    await ok();
    // A Desordem só existe no Mapa aleatório: no Summoner's Rift, o tipo volta para o primeiro.
    fireEvent.click(radio("Summoner's Rift"));
    expect(pick).toHaveValue('blindPick');
    expect(panel().queryByRole('combobox', { name: 'Seleção de mapas' })).not.toBeInTheDocument();
  });

  it('não mostra a aba Personalizada ao trocar a fila de um lobby', () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    fireEvent.click(button('Trocar modo'));
    expect(panel().queryByRole('tab', { name: 'Personalizada' })).not.toBeInTheDocument();
  });

  it('acinzenta o modo fechado e não deixa escolher', () => {
    render(<App />);
    hello({ queues: [{ ...gameQueue(1750, 'Arena 3x6', 'CHERRY'), available: false }] });
    expect(panel().getByRole('group', { name: 'Arena' })).toBeDisabled();
    expect(radio('Arena 3x6')).toBeDisabled();
    expect(panel().getByRole('group', { name: 'Arena' })).toHaveTextContent('Fechado agora.');
  });

  it('bloqueia o modo inteiro quando a fila atual é a única dele', () => {
    render(<App />);
    hello({ lobby: { ...testLobby, queue: null, queueId: 1750 }, gameflowPhase: 'Lobby', queues: [gameQueue(1750, 'Arena 3x6', 'CHERRY')] });
    fireEvent.click(button('Trocar modo'));
    expect(panel().getByRole('group', { name: 'Arena' })).toBeDisabled();
    expect(panel().getByRole('group', { name: 'Arena' })).toHaveTextContent('Você já está nesta fila.');
    expect(radio('Arena 3x6')).toBeDisabled();
    expect(radio('Partida Dinâmica')).toBeChecked();
  });

  it('troca uma fila fora da lista pela primeira aberta', () => {
    render(<App />);
    hello({ lobby: { ...testLobby, queue: null, queueId: 999 }, gameflowPhase: 'Lobby', queues: [desordem] });
    fireEvent.click(button('Trocar modo'));
    expect(panel().getByText('Você está no lobby de Outra fila (ID 999).')).toBeInTheDocument();
    expect(radio('Partida Dinâmica')).toBeChecked();
    fireEvent.click(button('Trocar fila'));
    expect(request().command).toEqual({ type: 'createLobby', queue: 'swiftplay' });
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

  it('não troca a fila depois que o LoL desconecta', () => {
    render(<App />);
    hello({ lobby: testLobby, gameflowPhase: 'Lobby' });
    fireEvent.click(button('Trocar modo'));
    fireEvent.click(radio('ARAM Normal'));
    act(() => socket().receive({ type: 'lcu', status: { state: 'disconnected', reason: 'fechado' } }));
    expect(panel().queryByRole('button', { name: 'Trocar fila' })).not.toBeInTheDocument();
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

  it.each([
    ['ARAM', { queue: 'aram' as const, queueId: 450 }],
    ['ARAM: Desordem', { queue: null, queueId: 2400 }],
    ['Arena 3x6', { queue: null, queueId: 1750 }],
    ['Co-op Introdução', { queue: null, queueId: 870 }],
    ['Jogo Dinâmico', { queue: 'swiftplay' as const, queueId: 480 }],
  ])('sem showPositionSelector, não mostra posições nem aviso no lobby do %s', (_, queue) => {
    render(<App />);
    hello({ lobby: { ...testLobby, ...queue, showPositionSelector: false }, gameflowPhase: 'Lobby' });
    expect(panel().queryByRole('combobox', { name: 'Posição primária' })).not.toBeInTheDocument();
    expect(panel().queryByText('Esta fila não permite escolher posições.')).not.toBeInTheDocument();
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

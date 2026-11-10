import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentClient } from './client';
import { testLobby } from '../test/lobby';
import {
  testChampSelect, testConversation, testFriend, testInvitation, testMatchmaking, testReadyCheck, testRuneCatalog, testRunePages,
  testSummonerSpellCatalog, testChampionCatalog,
} from '../test/events';
import type { CustomGameListing } from '../bindings/CustomGameListing';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelectChat } from '../bindings/ChampSelectChat';
import type { GameQueue } from '../bindings/GameQueue';
import type { MatchSummary } from '../bindings/MatchSummary';
import type { PlayerProfile } from '../bindings/PlayerProfile';
import type { ServerMessage } from '../bindings/ServerMessage';
import type { Command } from '../bindings/Command';
import type { CommandResult } from '../bindings/CommandResult';
import { makeState } from '../test/state';

class Socket {
  static OPEN = 1;
  static instances: Socket[] = [];
  readyState = 1;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  send = vi.fn();
  close = vi.fn();
  constructor() { Socket.instances.push(this); }
  receive(message: ServerMessage) { this.onmessage?.({ data: JSON.stringify(message) }); }
}
const testChampSelectChat: ChampSelectChat = {
  messages: [{ id: 'm1', body: 'vou top', fromMe: false, fromCellId: 0, timestamp: '2026-11-05T01:15:00.000Z' }],
};
const testQueue: GameQueue = {
  queueId: 2400, queue: null, name: 'ARAM: Desordem', description: 'ARAM: Desordem', gameMode: 'KIWI', modeGroup: 'kARAM',
  mapId: 12, versusAi: false, ranked: false, showPositionSelector: false, limitedTime: false, available: true,
};
const testMatch: MatchSummary = {
  gameId: 2001, queueId: 420, gameMode: 'CLASSIC', mapId: 11, createdAt: 1794326400000, durationSecs: 1800, result: 'win',
  player: {
    puuid: 'p', gameName: 'Jogador', tagLine: 'BR1', profileIconId: 29, teamId: 100, win: true, championId: 103,
    championLevel: 16, spell1Id: 4, spell2Id: 14, kills: 7, deaths: 2, assists: 9, cs: 180, gold: 12000,
    damageToChampions: 21000, visionScore: 22, items: [3071, 0, 0, 0, 0, 0, 3340], perkId: 8112, perkStyleId: 8100,
    perkSubStyleId: 8300,
  },
};
const unranked = { tier: null, division: null, leaguePoints: 0, wins: 0, losses: 0, provisional: false };
const testPlayerProfile: PlayerProfile = {
  summoner: { gameName: 'Amigo', tagLine: 'BR1', displayName: '', summonerLevel: 143, profileIconId: 29, puuid: 'amigo', summonerId: 1003 },
  privateProfile: false, ranked: { soloDuo: unranked, flex: unranked },
  championMasteries: [{ championId: 103, level: 12, points: 184000 }], matchHistory: { matches: [testMatch], hasMore: false },
};
const state: AgentState = makeState({ gameflowPhase: 'Lobby' });
const hello = (overrides: Partial<AgentState> = {}): ServerMessage => ({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...state, ...overrides } });
let client: AgentClient;
const socket = () => Socket.instances.at(-1)!;

beforeEach(() => {
  vi.useFakeTimers();
  Socket.instances = [];
  vi.stubGlobal('WebSocket', Socket);
  client = new AgentClient();
  client.start();
});
afterEach(() => { client.stop(); vi.useRealTimers(); vi.unstubAllGlobals(); });

const testCustomGame: CustomGameListing = {
  partyId: '06645d51-8137-48c7-9a29-e41cebf815dc', name: 'Partida de fake-name-0040', owner: 'fake-name-0040 #br1', mapId: 11,
  map: 'summonersRift', players: 1, maxPlayers: 10, spectators: 0, maxSpectators: 15, spectatorPolicy: 'all', hasPassword: true,
};

describe('canal do agente', () => {
  it('aguarda hello, aplica eventos e substitui o estado inteiro na ressincronização', () => {
    expect(client.getSnapshot().state).toBeNull();
    socket().receive(hello());
    socket().receive({ type: 'gameflowPhase', phase: 'ChampSelect' });
    expect(client.getSnapshot().state?.gameflowPhase).toBe('ChampSelect');
    socket().receive(hello({ gameflowPhase: null, lcu: { state: 'starting' } }));
    expect(client.getSnapshot().state).toEqual({ ...state, gameflowPhase: null, lcu: { state: 'starting' } });
  });
  it('aplica lobby no hello, eventos e ressincronização sem perder os outros campos', () => {
    socket().receive(hello({ lobby: testLobby }));
    expect(client.getSnapshot().state).toEqual({ ...state, lobby: testLobby });
    socket().receive({ type: 'lobby', lobby: { ...testLobby, queue: null, queueId: 999 } });
    expect(client.getSnapshot().state?.lobby?.queueId).toBe(999);
    socket().receive({ type: 'lobby', lobby: null });
    expect(client.getSnapshot().state).toEqual(state);
    socket().receive({ type: 'lobby', lobby: testLobby });
    socket().receive(hello());
    expect(client.getSnapshot().state?.lobby).toBeNull();
  });
  it.each([
    undefined, {}, { ...testLobby, queue: 'tft' }, { ...testLobby, queue: 'toString' },
    { ...testLobby, queueId: -1 }, { ...testLobby, maxLobbySize: '5' },
    { ...testLobby, isLeader: 'true' }, { ...testLobby, canStartActivity: null },
    { ...testLobby, showPositionSelector: null }, { ...testLobby, members: [null] },
    { ...testLobby, members: [{ ...testLobby.members[0], secondPosition: 'toString' }] },
  ])('rejeita lobby malformado tanto no evento quanto no hello: %j', lobby => {
    socket().receive(hello());
    socket().onmessage?.({ data: JSON.stringify({ type: 'lobby', lobby }) });
    expect(client.getSnapshot().status).toBe('reconnecting');
    vi.advanceTimersByTime(1000);
    socket().onmessage?.({ data: JSON.stringify({ ...hello(), state: { ...state, lobby } }) });
    expect(client.getSnapshot().status).toBe('reconnecting');
  });
  it('limpa dados na queda e reconecta sem reaproveitar mensagens antigas', () => {
    socket().receive(hello());
    const old = socket();
    const late = old.onmessage;
    old.onclose?.();
    expect(client.getSnapshot().state).toBeNull();
    vi.advanceTimersByTime(1000);
    expect(Socket.instances).toHaveLength(2);
    late?.({ data: JSON.stringify(hello()) });
    expect(client.getSnapshot().state).toBeNull();
    socket().receive(hello());
    expect(client.getSnapshot().status).toBe('connected');
  });
  it('correlaciona respostas fora de ordem e envia comandos no contrato gerado', async () => {
    socket().receive(hello());
    const first = client.send({ type: 'ping' });
    const second = client.send({ type: 'getState' });
    expect(JSON.parse(socket().send.mock.calls[1][0])).toEqual({ id: 2, command: { type: 'getState' } });
    socket().receive({ type: 'response', id: 2, result: { status: 'error', code: 'lcu', message: 'falhou' } });
    socket().receive({ type: 'response', id: 1, result: { status: 'ok' } });
    await expect(first).resolves.toEqual({ status: 'ok' });
    await expect(second).resolves.toEqual({ status: 'error', code: 'lcu', message: 'falhou' });
  });
  it.each([
    { command: { type: 'launchLeague' }, result: { status: 'error', code: 'launchFailed', message: 'Riot Client não encontrado' } },
    { command: { type: 'setPositions', first: 'middle', second: 'middle' }, result: { status: 'error', code: 'invalidCommand', message: 'Os cargos devem ser diferentes' } },
    { command: { type: 'launchUx' }, result: { status: 'error', code: 'notConnected', message: 'LoL fechado' } },
    { command: { type: 'launchUx' }, result: { status: 'error', code: 'lcu', message: 'A LCU recusou o comando' } },
  ] satisfies { command: Command; result: CommandResult }[])('entrega $result.code sem alterar estado nem reconectar', async ({ command, result }) => {
    const connection = socket();
    connection.receive(hello());
    const snapshot = client.getSnapshot();
    const listener = vi.fn();
    const unsubscribe = client.subscribe(listener);
    const response = client.send(command);
    expect(JSON.parse(connection.send.mock.calls[0][0])).toEqual({ id: 1, command });
    connection.receive({ type: 'response', id: 1, result });
    await expect(response).resolves.toEqual(result);

    expect(client.getSnapshot()).toBe(snapshot);
    expect(client.getSnapshot().status).toBe('connected');
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(60000);
    expect(client.getSnapshot()).toBe(snapshot);
    expect(listener).not.toHaveBeenCalled();
    expect(connection.close).not.toHaveBeenCalled();
    expect(Socket.instances).toHaveLength(1);

    const next = client.send({ type: 'ping' });
    expect(JSON.parse(connection.send.mock.calls[1][0])).toEqual({ id: 2, command: { type: 'ping' } });
    connection.receive({ type: 'response', id: 2, result: { status: 'ok' } });
    await expect(next).resolves.toEqual({ status: 'ok' });
    unsubscribe();
  });
  it.each([
    ['JSON inválido', '{'],
    ...[
      ['código desconhecido', { status: 'error', code: 'unknown', message: 'erro' }],
      ['código herdado', { status: 'error', code: 'toString', message: 'erro' }],
      ['código em array', { status: 'error', code: ['lcu'], message: 'erro' }],
      ['código ausente', { status: 'error', message: 'erro' }],
      ['código nulo', { status: 'error', code: null, message: 'erro' }],
      ['mensagem ausente', { status: 'error', code: 'launchFailed' }],
      ['mensagem não textual', { status: 'error', code: 'invalidCommand', message: 42 }],
      ['status inválido', { status: 'failed', code: 'launchFailed', message: 'erro' }],
      ['resultado nulo', null],
    ].map(([name, result]) => [String(name), JSON.stringify({ type: 'response', id: 1, result })]),
    ['id inválido', JSON.stringify({ type: 'response', id: '1', result: { status: 'error', code: 'launchFailed', message: 'erro' } })],
  ])('rejeita resposta malformada: %s', async (_name, data) => {
    const connection = socket();
    connection.receive(hello());
    const response = expect(client.send({ type: 'ping' })).rejects.toThrow('Conexão com o agente encerrada.');
    connection.onmessage?.({ data });
    await response;
    expect(client.getSnapshot()).toEqual({ status: 'reconnecting', state: null, error: 'Recebemos uma mensagem inválida do agente.' });
    expect(connection.close).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(1000);
    expect(Socket.instances).toHaveLength(2);
  });
  it('rejeita comando sem conexão e limita o tempo de resposta', async () => {
    await expect(client.send({ type: 'ping' })).rejects.toThrow('não está conectado');
    socket().receive(hello());
    const result = expect(client.send({ type: 'ping' })).rejects.toThrow('não respondeu');
    vi.advanceTimersByTime(10000);
    await result;
  });
  it('rejeita comandos pendentes na desconexão', async () => {
    socket().receive(hello());
    const result = expect(client.send({ type: 'ping' })).rejects.toThrow('encerrada');
    socket().onclose?.();
    await result;
  });
  it('interrompe conexão incompatível sem loop de reconexão', () => {
    socket().receive({ type: 'hello', protocol: 2, agentVersion: '2', state });
    expect(client.getSnapshot().status).toBe('incompatible');
    vi.advanceTimersByTime(60000);
    expect(Socket.instances).toHaveLength(1);
  });
  it('recupera mensagens malformadas sem manter estado velho', () => {
    socket().receive(hello());
    socket().onmessage?.({ data: '{"type":"hello","protocol":1,"state":{}}' });
    expect(client.getSnapshot().status).toBe('reconnecting');
    expect(client.getSnapshot().state).toBeNull();
  });
  it('ignora eventos futuros e aceita campos adicionais', () => {
    socket().onmessage?.({ data: JSON.stringify({ ...hello(), extra: true }) });
    socket().onmessage?.({ data: '{"type":"futureEvent"}' });
    expect(client.getSnapshot().state).toEqual(state);
  });
  it('reconecta quando não recebe hello e cancela timers ao desmontar', () => {
    vi.advanceTimersByTime(10000);
    expect(client.getSnapshot().status).toBe('reconnecting');
    client.stop();
    vi.advanceTimersByTime(60000);
    expect(Socket.instances).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
  });
  it('suporta o ciclo mount/unmount/mount do StrictMode sem duplicar sockets', () => {
    const old = socket();
    client.stop();
    client.start();
    expect(old.close).toHaveBeenCalledOnce();
    expect(Socket.instances).toHaveLength(2);
    socket().receive(hello());
    expect(client.getSnapshot().status).toBe('connected');
  });
});

describe('eventos de estado do agente', () => {
  it.each([
    [{ type: 'readyCheck', readyCheck: testReadyCheck }, { readyCheck: testReadyCheck }],
    [{ type: 'matchmaking', matchmaking: testMatchmaking }, { matchmaking: testMatchmaking }],
    [{ type: 'champSelect', champSelect: testChampSelect }, { champSelect: testChampSelect }],
    [{ type: 'receivedInvitations', invitations: [testInvitation] }, { receivedInvitations: [testInvitation] }],
    [{ type: 'runePages', runePages: testRunePages }, { runePages: testRunePages }],
    [{ type: 'runeCatalog', runeCatalog: testRuneCatalog }, { runeCatalog: testRuneCatalog }],
    [{ type: 'summonerSpellCatalog', summonerSpellCatalog: testSummonerSpellCatalog }, { summonerSpellCatalog: testSummonerSpellCatalog }],
    [{ type: 'championCatalog', championCatalog: testChampionCatalog }, { championCatalog: testChampionCatalog }],
    [{ type: 'champSelectChampions', champSelectChampions: { pickable: [1, 266], bannable: null, subset: null } },
      { champSelectChampions: { pickable: [1, 266], bannable: null, subset: null } }],
    [{ type: 'friends', friends: [testFriend] }, { friends: [testFriend] }],
    [{ type: 'queues', queues: [testQueue] }, { queues: [testQueue] }],
    [{ type: 'ranked', ranked: { soloDuo: { ...unranked, tier: 'GOLD', division: 'II' }, flex: unranked } },
      { ranked: { soloDuo: { ...unranked, tier: 'GOLD', division: 'II' }, flex: unranked } }],
    [{ type: 'championMasteries', championMasteries: [{ championId: 103, level: 23, points: 287450 }] },
      { championMasteries: [{ championId: 103, level: 23, points: 287450 }] }],
    [{ type: 'matchHistory', matchHistory: { matches: [testMatch], hasMore: true } },
      { matchHistory: { matches: [testMatch], hasMore: true } }],
    [{ type: 'matchDetails', matchDetails: { ...testMatch, remake: false, players: [testMatch.player] } },
      { matchDetails: { ...testMatch, remake: false, players: [testMatch.player] } }],
    [{ type: 'playerProfile', playerProfile: testPlayerProfile }, { playerProfile: testPlayerProfile }],
    [{ type: 'customGames', customGames: [testCustomGame] }, { customGames: [testCustomGame] }],
    [{ type: 'patchNotes', patchNotes: { version: '26.22', title: 'Notas', summary: null, url: 'https://www.leagueoflegends.com/', sections: [] } },
      { patchNotes: { version: '26.22', title: 'Notas', summary: null, url: 'https://www.leagueoflegends.com/', sections: [] } }],
    [{ type: 'wallet', wallet: { blueEssence: 48210, orangeEssence: 3120, rp: null } },
      { wallet: { blueEssence: 48210, orangeEssence: 3120, rp: null } }],
  ] satisfies [ServerMessage, Partial<AgentState>][])('aplica %j sem perder os outros campos', (message, change) => {
    socket().receive(hello({ lobby: testLobby }));
    socket().receive(message);
    expect(client.getSnapshot()).toEqual({ status: 'connected', state: { ...state, lobby: testLobby, ...change }, error: null });
  });
  it.each([
    [{ type: 'readyCheck', readyCheck: null }, { readyCheck: null }],
    [{ type: 'matchmaking', matchmaking: null }, { matchmaking: null }],
    [{ type: 'champSelect', champSelect: null }, { champSelect: null, champSelectChat: null }],
    [{ type: 'receivedInvitations', invitations: [] }, { receivedInvitations: [] }],
    [{ type: 'runePages', runePages: null }, { runePages: null }],
    [{ type: 'runeCatalog', runeCatalog: null }, { runeCatalog: null }],
    [{ type: 'summonerSpellCatalog', summonerSpellCatalog: null }, { summonerSpellCatalog: null }],
    [{ type: 'championCatalog', championCatalog: null }, { championCatalog: null }],
    [{ type: 'champSelectChampions', champSelectChampions: { pickable: null, bannable: null, subset: null } },
      { champSelectChampions: { pickable: null, bannable: null, subset: null } }],
    [{ type: 'friends', friends: [] }, { friends: [] }],
    [{ type: 'champSelectChat', champSelectChat: null }, { champSelectChat: null }],
  ] satisfies [ServerMessage, Partial<AgentState>][])('limpa o campo com %j', (message, change) => {
    const full: Partial<AgentState> = {
      readyCheck: testReadyCheck, matchmaking: testMatchmaking, champSelect: testChampSelect, receivedInvitations: [testInvitation],
      runePages: testRunePages, runeCatalog: testRuneCatalog, summonerSpellCatalog: testSummonerSpellCatalog, friends: [testFriend],
      champSelectChat: testChampSelectChat, championCatalog: testChampionCatalog,
      champSelectChampions: { pickable: [1], bannable: [1, 2], subset: null },
    };
    socket().receive(hello(full));
    socket().receive(message);
    expect(client.getSnapshot().state).toEqual({ ...state, ...full, ...change });
  });
  it('troca a conversa de mesmo id, acrescenta as novas e tira as fechadas', () => {
    const other = { ...testConversation, id: 'outro@br1.pvp.net', gameName: 'Outro', messages: [] };
    socket().receive(hello({ conversations: [testConversation, other] }));
    const updated = { ...testConversation, unreadCount: 0, messages: [...testConversation.messages,
      { id: 'm2', body: 'bora', fromMe: true, timestamp: '2026-11-05T01:14:00.000Z' }] };
    socket().receive({ type: 'conversation', conversation: updated });
    expect(client.getSnapshot().state?.conversations).toEqual([updated, other]);
    const fresh = { ...testConversation, id: 'novo@br1.pvp.net', gameName: 'Novo' };
    socket().receive({ type: 'conversation', conversation: fresh });
    expect(client.getSnapshot().state?.conversations).toEqual([updated, other, fresh]);
    socket().receive({ type: 'conversationClosed', id: other.id });
    expect(client.getSnapshot().state?.conversations).toEqual([updated, fresh]);
    socket().receive({ type: 'conversationClosed', id: 'desconhecida' });
    expect(client.getSnapshot().state?.conversations).toEqual([updated, fresh]);
  });
  it('substitui os dados dos eventos na ressincronização', () => {
    socket().receive(hello());
    socket().receive({ type: 'friends', friends: [testFriend] });
    socket().receive({ type: 'conversation', conversation: testConversation });
    socket().receive(hello());
    expect(client.getSnapshot().state).toEqual(state);
  });
  it.each([
    ['readyCheck', { type: 'readyCheck', readyCheck: { ...testReadyCheck, timer: '2' } }],
    ['readyCheck sem campo', { type: 'readyCheck' }],
    ['matchmaking', { type: 'matchmaking', matchmaking: { ...testMatchmaking, errors: null } }],
    ['champSelect', { type: 'champSelect', champSelect: { ...testChampSelect, id: 1 } }],
    ['champSelect com fila herdada', { type: 'champSelect', champSelect: { ...testChampSelect, queue: 'toString' } }],
    ['champSelect sem bans', { type: 'champSelect', champSelect: { ...testChampSelect, bans: null } }],
    ['convites', { type: 'receivedInvitations', invitations: [{ ...testInvitation, canAccept: 'sim' }] }],
    ['convites nulos', { type: 'receivedInvitations', invitations: null }],
    ['convites do lobby', { type: 'lobby', lobby: { ...testLobby, invitations: [{ puuid: 'a', summonerId: 1, state: 'Talvez' }] } }],
    ['lobby sem canInvite', { type: 'lobby', lobby: { ...testLobby, canInvite: undefined } }],
    ['conversa', { type: 'conversation', conversation: { ...testConversation, id: null } }],
    ['conversa nula', { type: 'conversation', conversation: null }],
    ['conversa fechada', { type: 'conversationClosed', id: 3 }],
    ['runas', { type: 'runePages', runePages: { ...testRunePages, pages: [null] } }],
    ['catálogo', { type: 'runeCatalog', runeCatalog: { ...testRuneCatalog, perks: null } }],
    ['amigos', { type: 'friends', friends: [{ ...testFriend, id: undefined }] }],
  ])('rejeita evento malformado: %s', (_name, message) => {
    socket().receive(hello());
    socket().onmessage?.({ data: JSON.stringify(message) });
    expect(client.getSnapshot()).toEqual({ status: 'reconnecting', state: null, error: 'Recebemos uma mensagem inválida do agente.' });
  });
  it.each([
    ['readyCheck', { readyCheck: {} }],
    ['matchmaking', { matchmaking: [] }],
    ['champSelect', { champSelect: { ...testChampSelect, timer: null } }],
    ['convites', { receivedInvitations: undefined }],
    ['conversas', { conversations: [{ ...testConversation, messages: null }] }],
    ['runas', { runePages: { ...testRunePages, currentPageId: -1 } }],
    ['catálogo', { runeCatalog: { styles: [] } }],
    ['amigos', { friends: null }],
  ])('rejeita hello com %s malformado', (_name, change) => {
    socket().onmessage?.({ data: JSON.stringify({ ...hello(), state: { ...state, ...change } }) });
    expect(client.getSnapshot().status).toBe('reconnecting');
  });
});

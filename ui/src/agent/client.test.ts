import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentClient } from './client';
import type { AgentState } from '../bindings/AgentState';
import type { ServerMessage } from '../bindings/ServerMessage';

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
const state: AgentState = { lcu: { state: 'connected', session: 1 }, summoner: null, gameflowPhase: 'Lobby', readyCheck: null, lobby: null, matchmaking: null };
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

describe('canal do agente', () => {
  it('aguarda hello, aplica eventos e substitui o estado inteiro na ressincronização', () => {
    expect(client.getSnapshot().state).toBeNull();
    socket().receive(hello());
    socket().receive({ type: 'gameflowPhase', phase: 'ChampSelect' });
    expect(client.getSnapshot().state?.gameflowPhase).toBe('ChampSelect');
    socket().receive(hello({ gameflowPhase: null, lcu: { state: 'starting' } }));
    expect(client.getSnapshot().state).toEqual({ ...state, gameflowPhase: null, lcu: { state: 'starting' } });
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

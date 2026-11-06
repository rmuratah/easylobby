import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testReadyCheck, testMatchmaking } from '../test/events';
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
const panel = () => within(screen.getByRole('region', { name: 'Partida encontrada' }));
const button = (name: string) => panel().getByRole('button', { name });
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: request().id, result }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

const open = () => hello({ gameflowPhase: 'ReadyCheck', readyCheck: testReadyCheck, lobby: testLobby });
const event = (change: Partial<typeof testReadyCheck> = {}) => act(() => socket().receive({ type: 'readyCheck', readyCheck: { ...testReadyCheck, ...change } }));

it.each([['Aceitar', 'acceptReadyCheck', 'Accepted', 'Aceitou'], ['Recusar', 'declineReadyCheck', 'Declined', 'Recusou']] as const)(
  '%s aguarda evento, antes ou depois da resposta, sem duplicar', async (label, command, response, text) => {
    render(<App />); open();
    act(() => { fireEvent.click(button(label)); fireEvent.click(button(label)); });
    expect(socket().send).toHaveBeenCalledTimes(1);
    expect(request().command.type).toBe(command);
    expect(panel().getByText('Ainda não respondeu')).toBeInTheDocument();
    if (response === 'Accepted') await respond();
    expect(button(label)).toBeDisabled();
    event({ playerResponse: response });
    if (response === 'Declined') await respond({ status: 'error', code: 'lcu', message: 'antigo' });
    expect(panel().getByText(text)).toBeInTheDocument();
    expect(panel().queryByText(/antigo/)).not.toBeInTheDocument();
    expect(button('Aceitar')).toBeDisabled(); expect(button('Recusar')).toBeDisabled();
  });
it('ressincroniza duration e timer e limita em zero', () => {
  render(<App />); open();
  event({ duration: 20, timer: 2 });
  expect(panel().getByLabelText('Tempo para responder')).toHaveTextContent('18 s');
  act(() => vi.advanceTimersByTime(3000));
  expect(panel().getByLabelText('Tempo para responder')).toHaveTextContent('15 s');
  event({ duration: 30, timer: 10 });
  expect(panel().getByLabelText('Tempo para responder')).toHaveTextContent('20 s');
  act(() => vi.advanceTimersByTime(40000));
  expect(panel().getByLabelText('Tempo para responder')).toHaveTextContent('0 s');
});
it.each(['EveryoneReady', 'StrangerNotReady', 'PartyNotReady', 'Error', 'Unknown', 'Invalid'] as const)('mantém resultado %s até null', state => {
  render(<App />); open(); event({ state });
  expect(button('Aceitar')).toBeDisabled();
  expect(panel().queryByLabelText('Tempo para responder')).not.toBeInTheDocument();
  act(() => socket().receive({ type: 'readyCheck', readyCheck: null }));
  expect(screen.queryByRole('region', { name: 'Partida encontrada' })).not.toBeInTheDocument();
});
it('bloqueia resposta desconhecida e ações incompatíveis mesmo antes da fase', () => {
  render(<App />); open(); event({ playerResponse: 'Unknown' });
  expect(panel().getByText('Resposta desconhecida')).toBeInTheDocument();
  expect(button('Aceitar')).toBeDisabled();
  hello({ gameflowPhase: 'Lobby', lobby: testLobby, readyCheck: testReadyCheck });
  expect(screen.getByRole('button', { name: 'Iniciar busca' })).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Sair do lobby' })).toBeDisabled();
  hello({ gameflowPhase: 'Matchmaking', matchmaking: testMatchmaking, readyCheck: testReadyCheck });
  expect(screen.getByRole('button', { name: 'Cancelar busca' })).toBeDisabled();
});
it.each(['Lobby', 'ChampSelect'] as const)('limpa timer e descarta resposta ao mudar para %s', async phase => {
  render(<App />); open(); fireEvent.click(button('Aceitar'));
  act(() => socket().receive({ type: 'gameflowPhase', phase }));
  await respond({ status: 'error', code: 'lcu', message: 'antigo' });
  expect(panel().queryByText(/antigo/)).not.toBeInTheDocument();
  expect(panel().queryByLabelText('Tempo para responder')).not.toBeInTheDocument();
  expect(button('Aceitar')).toBeDisabled();
  expect(vi.getTimerCount()).toBe(0);
});
it('trata erro LCU, falha de envio e não repete envio incerto', async () => {
  render(<App />); open(); fireEvent.click(button('Aceitar'));
  await respond({ status: 'error', code: 'lcu', message: 'LCU indisponível' });
  expect(panel().getByRole('alert')).toHaveTextContent('LCU indisponível');
  socket().send.mockImplementationOnce(() => { throw new Error('envio'); });
  await act(async () => fireEvent.click(button('Recusar')));
  expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível confirmar');
  expect(button('Aceitar')).toBeDisabled();
});
it('timeout mantém bloqueio até evento confirmado', async () => {
  render(<App />); open(); fireEvent.click(button('Aceitar'));
  await act(async () => vi.advanceTimersByTime(10000));
  expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível confirmar');
  fireEvent.click(button('Recusar')); expect(socket().send).toHaveBeenCalledTimes(1);
  event({ playerResponse: 'Accepted' });
  expect(panel().getByText('Aceitou')).toBeInTheDocument();
});
it.each(['null', 'session', 'lcu', 'socket', 'unmount'])('descarta resposta antiga após %s', async transition => {
  const view = render(<App />); open(); fireEvent.click(button('Aceitar'));
  const old = socket(); const late = old.onmessage; const id = request().id;
  await act(async () => {
    if (transition === 'null') old.receive({ type: 'readyCheck', readyCheck: null });
    if (transition === 'session') old.receive({ type: 'lcu', status: { state: 'connected', session: 99 } });
    if (transition === 'lcu') old.receive({ type: 'lcu', status: { state: 'disconnected', reason: 'queda' } });
    if (transition === 'socket') old.onclose?.();
    if (transition === 'unmount') view.unmount();
  });
  if (transition === 'socket') act(() => vi.advanceTimersByTime(1000));
  if (transition !== 'unmount') open();
  await act(async () => late?.({ data: JSON.stringify({ type: 'response', id, result: { status: 'error', code: 'lcu', message: 'antigo' } }) }));
  if (transition === 'socket') act(() => late?.({ data: JSON.stringify({ type: 'readyCheck', readyCheck: null }) }));
  expect(screen.queryByText(/antigo/)).not.toBeInTheDocument();
  if (transition !== 'unmount') expect(button('Aceitar')).toBeEnabled();
  view.unmount(); expect(vi.getTimerCount()).toBe(0);
});

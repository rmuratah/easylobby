import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelectChat } from '../bindings/ChampSelectChat';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import { champSelectState } from '../test/champSelect';

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
const chat = () => within(screen.getByRole('region', { name: 'Chat do time' }));
const input = () => chat().getByRole('textbox', { name: 'Mensagem para o time' });
const last = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const room: ChampSelectChat = {
  messages: [
    { id: 'm1', body: 'vou de jungle', fromMe: false, fromCellId: 1, timestamp: '2026-11-05T01:13:41.755Z' },
    { id: 'm2', body: 'quem é?', fromMe: false, fromCellId: null, timestamp: '2026-11-05T01:14:00.000Z' },
  ],
};
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...champSelectState, champSelectChat: room, ...overrides } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: last().id, result }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('mostra as mensagens com quem mandou e recebe as novas', () => {
  render(<App />); hello();
  const log = within(chat().getByRole('log', { name: 'Mensagens do time' }));
  expect(log.getByText('vou de jungle').closest('li')).toHaveTextContent('Amigo#BR1: vou de jungle');
  expect(log.getByText('quem é?').closest('li')).toHaveTextContent('Aliado: quem é?');
  act(() => socket().receive({
    type: 'champSelectChat',
    champSelectChat: { messages: [...room.messages, { id: 'm3', body: 'gg', fromMe: true, fromCellId: 0, timestamp: '2026-11-05T01:15:00.000Z' }] },
  }));
  expect(log.getByText('gg').closest('li')).toHaveTextContent('Você: gg');
});

it('manda mensagem, limpa o campo e mostra erro sem perder o texto', async () => {
  render(<App />); hello();
  expect(chat().getByRole('button', { name: 'Enviar' })).toBeDisabled();
  fireEvent.change(input(), { target: { value: '  bora  ' } });
  fireEvent.submit(input());
  expect(last().command).toEqual({ type: 'sendChampSelectMessage', body: 'bora' });
  expect(chat().getByRole('button', { name: 'Enviando…' })).toBeDisabled();
  await respond();
  expect(input()).toHaveValue('');
  fireEvent.change(input(), { target: { value: 'de novo' } });
  fireEvent.submit(input());
  await respond({ status: 'error', code: 'lcu', message: 'sala fechada' });
  expect(chat().getByRole('alert')).toHaveTextContent('Não foi possível enviar a mensagem: sala fechada');
  expect(input()).toHaveValue('de novo');
});

it('espera a sala abrir', () => {
  render(<App />); hello({ champSelectChat: null });
  expect(chat().getByText('Aguardando a sala do chat abrir…')).toBeVisible();
  expect(input()).toBeDisabled();
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: { messages: [] } }));
  expect(chat().getByText('Nenhuma mensagem ainda.')).toBeVisible();
  expect(input()).toBeEnabled();
});

it('não envia espaços nem duplica submissões e só mostra mensagens confirmadas', async () => {
  render(<App />); hello();
  fireEvent.change(input(), { target: { value: '   ' } });
  fireEvent.submit(input());
  expect(socket().send).not.toHaveBeenCalled();
  fireEvent.change(input(), { target: { value: 'bora' } });
  act(() => { fireEvent.submit(input()); fireEvent.submit(input()); });
  expect(socket().send).toHaveBeenCalledOnce();
  expect(chat().queryByText('bora')).not.toBeInTheDocument();
  const confirmed: ChampSelectChat = { messages: [...room.messages,
    { id: 'm3', body: 'bora', fromMe: true, fromCellId: 0, timestamp: room.messages[0].timestamp }] };
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: confirmed }));
  await respond();
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: confirmed }));
  expect(chat().getAllByText('bora')).toHaveLength(1);
  expect(chat().getAllByRole('listitem').map(li => li.textContent)).toEqual([
    expect.stringContaining('vou de jungle'), expect.stringContaining('quem é?'), expect.stringContaining('bora'),
  ]);
});

it('mantém um novo rascunho digitado durante o envio', async () => {
  render(<App />); hello();
  fireEvent.change(input(), { target: { value: 'primeiro' } });
  fireEvent.submit(input());
  fireEvent.change(input(), { target: { value: 'segundo' } });
  await respond();
  expect(input()).toHaveValue('segundo');
});

it('trata timeout como resultado incerto, sem reenvio automático, e aceita confirmação tardia', async () => {
  render(<App />); hello();
  fireEvent.change(input(), { target: { value: 'atrasada' } });
  fireEvent.submit(input());
  await act(async () => vi.advanceTimersByTime(10000));
  expect(chat().getByRole('alert')).toHaveTextContent('Confira o histórico antes de tentar novamente');
  expect(input()).toHaveValue('atrasada');
  expect(socket().send).toHaveBeenCalledOnce();
  await respond();
  expect(input()).toHaveValue('atrasada');
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: { messages: [
    { ...room.messages[0], id: 'late', body: 'atrasada', fromMe: true },
  ] } }));
  expect(chat().getByText('atrasada')).toBeVisible();
});

it('fecha a sala, descarta o rascunho e ignora resposta do envio antigo ao reabrir', async () => {
  render(<App />); hello();
  fireEvent.change(input(), { target: { value: 'antiga' } });
  fireEvent.submit(input());
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: null }));
  expect(input()).toBeDisabled();
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: { messages: [] } }));
  fireEvent.change(input(), { target: { value: 'nova' } });
  await respond({ status: 'error', code: 'lcu', message: 'erro antigo' });
  expect(input()).toHaveValue('nova');
  expect(chat().queryByRole('alert')).not.toBeInTheDocument();
});

it.each(['seleção', 'conexão LCU', 'fase', 'sessão encerrada', 'socket'] as const)(
  'invalida o envio ao mudar %s e não carrega mensagens antigas', async kind => {
    render(<App />); hello();
    fireEvent.change(input(), { target: { value: 'antiga' } });
    fireEvent.submit(input());
    const old = last();
    const oldSocket = socket();
    await act(async () => {
      if (kind === 'seleção') socket().receive({ type: 'champSelect', champSelect: { ...champSelectState.champSelect!, id: 'nova' } });
      if (kind === 'conexão LCU') socket().receive({ type: 'lcu', status: { state: 'connected', session: 2 } });
      if (kind === 'fase') socket().receive({ type: 'gameflowPhase', phase: 'GameStart' });
      if (kind === 'sessão encerrada') socket().receive({ type: 'champSelect', champSelect: null });
      if (kind === 'socket') oldSocket.onclose?.();
    });
    expect(screen.queryByText('vou de jungle')).not.toBeInTheDocument();
    if (kind === 'socket') act(() => vi.advanceTimersByTime(1000));
    hello({ champSelectChat: { messages: [] }, champSelect: { ...champSelectState.champSelect!, id: 'nova' } });
    fireEvent.change(input(), { target: { value: 'nova' } });
    await act(async () => oldSocket.receive({ type: 'response', id: old.id, result: { status: 'error', code: 'lcu', message: 'erro antigo' } }));
    expect(input()).toHaveValue('nova');
    expect(chat().queryByRole('alert')).not.toBeInTheDocument();
  },
);

it('ignora chat atrasado depois da fase terminar ou da LCU desconectar', () => {
  render(<App />); hello();
  act(() => socket().receive({ type: 'gameflowPhase', phase: 'GameStart' }));
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: room }));
  act(() => socket().receive({ type: 'gameflowPhase', phase: 'ChampSelect' }));
  expect(input()).toBeDisabled();
  act(() => socket().receive({ type: 'lcu', status: { state: 'disconnected', reason: 'LoL fechado' } }));
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: room }));
  act(() => socket().receive({ type: 'lcu', status: { state: 'connected', session: 3 } }));
  expect(input()).toBeDisabled();
});

it('acompanha o fim, preserva a leitura anterior e volta a acompanhar ao descer', () => {
  render(<App />); hello();
  const log = chat().getByRole('log');
  let height = 1000;
  Object.defineProperties(log, { scrollHeight: { get: () => height }, clientHeight: { value: 200 } });
  const update = (id: string) => act(() => socket().receive({ type: 'champSelectChat', champSelectChat: {
    messages: [...room.messages, { ...room.messages[0], id, body: id }],
  } }));
  update('m3');
  expect(log.scrollTop).toBe(1000);
  log.scrollTop = 100; fireEvent.scroll(log);
  height = 1100; update('m4');
  expect(log.scrollTop).toBe(100);
  log.scrollTop = 900; fireEvent.scroll(log);
  height = 1200; update('m5');
  expect(log.scrollTop).toBe(1200);
});

it('não revive mensagens depois de a sessão ser apagada', () => {
  render(<App />); hello();
  act(() => socket().receive({ type: 'champSelect', champSelect: null }));
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: room }));
  act(() => socket().receive({ type: 'champSelect', champSelect: { ...champSelectState.champSelect!, id: 'nova' } }));
  expect(input()).toBeDisabled();
  expect(chat().queryByText('vou de jungle')).not.toBeInTheDocument();
});

it('identifica aliados anônimos e acompanha a célula resolvida pelo agente', () => {
  render(<App />);
  const selection = champSelectState.champSelect!;
  const anonymous = { ...selection.myTeam[1], gameName: null, tagLine: null };
  hello({ champSelect: { ...selection, myTeam: [selection.myTeam[0], anonymous] } });
  expect(chat().getByText('vou de jungle').closest('li')).toHaveTextContent('Aliado 2');
  act(() => socket().receive({ type: 'champSelect', champSelect: {
    ...selection, myTeam: [{ ...selection.myTeam[1], cellId: 3 }, selection.myTeam[0]],
  } }));
  act(() => socket().receive({ type: 'champSelectChat', champSelectChat: {
    messages: [{ ...room.messages[0], fromCellId: 3 }],
  } }));
  expect(chat().getByText('vou de jungle').closest('li')).toHaveTextContent('Amigo#BR1');
});

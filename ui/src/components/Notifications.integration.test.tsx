import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { Conversation } from '../bindings/Conversation';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testConversation, testInvitation, testReadyCheck } from '../test/events';
import { testState } from '../test/lobby';
import { draftChampSelect } from '../test/champSelect';
import { MESSAGE_NOTICE_MS } from './Notifications';
import { SIDEBAR_STORAGE_KEY } from '../layout/sidebar';
import { sound } from '../agent/sound';

vi.mock('../agent/sound', () => ({ sound: { unlock: vi.fn(), playMatchFound: vi.fn() } }));

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
const receive = (message: ServerMessage) => act(() => socket().receive(message));
const hello = (overrides: Partial<AgentState> = {}) =>
  receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } });
const region = () => screen.queryByRole('region', { name: 'Notificações' });
const notices = () => within(region()!);
const chat = (change: Partial<Conversation>) => receive({ type: 'conversation', conversation: { ...testConversation, ...change } });
const newMessage = { id: 'm2', body: 'joga uma?', fromMe: false, timestamp: '2026-11-05T01:14:00.000Z' };

beforeEach(() => { localStorage.clear(); vi.clearAllMocks(); vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('não avisa o histórico que já existia ao conectar', () => {
  render(<App />);
  hello({ conversations: [testConversation] });
  expect(region()).not.toBeInTheDocument();
});

it('avisa mensagem nova e some sozinho depois de um tempo', () => {
  render(<App />);
  hello({ conversations: [testConversation] });
  chat({ unreadCount: 2, messages: [...testConversation.messages, newMessage] });
  expect(notices().getByText('Mensagem de Amigo#BR1')).toBeInTheDocument();
  expect(notices().getByText('joga uma?')).toBeInTheDocument();
  act(() => vi.advanceTimersByTime(MESSAGE_NOTICE_MS - 1000));
  chat({ unreadCount: 3, messages: [...testConversation.messages, newMessage, { ...newMessage, id: 'm3', body: 'bora' }] });
  expect(notices().getByText('2 mensagens de Amigo#BR1')).toBeInTheDocument();
  act(() => vi.advanceTimersByTime(MESSAGE_NOTICE_MS - 1000));
  expect(notices().getByText('bora')).toBeInTheDocument();
  act(() => vi.advanceTimersByTime(1000));
  expect(region()).not.toBeInTheDocument();
});

it('tira o aviso de mensagem quando a conversa é lida', () => {
  render(<App />);
  hello({ conversations: [testConversation] });
  chat({ unreadCount: 2, messages: [...testConversation.messages, newMessage] });
  chat({ unreadCount: 0, messages: [...testConversation.messages, newMessage] });
  expect(region()).not.toBeInTheDocument();
});

it('com a coluna aberta, o convite fica só nela', () => {
  render(<App />);
  hello({ receivedInvitations: [testInvitation] });
  expect(region()).not.toBeInTheDocument();
  expect(within(screen.getByRole('region', { name: 'Convites recebidos' })).getByText(/chamou você/)).toBeInTheDocument();
});

it('avisa convite enquanto ele está pendente e deixa fechar', () => {
  // Com a coluna recolhida, o aviso flutuante é o que mostra o convite.
  localStorage.setItem(SIDEBAR_STORAGE_KEY, 'recolhida');
  render(<App />);
  hello();
  receive({ type: 'receivedInvitations', invitations: [testInvitation, { ...testInvitation, invitationId: 'convite-2', fromSummonerName: 'Victor', queue: 'aram' }] });
  expect(notices().getByText('Um amigo convidou você para Ranqueada Solo/Duo')).toBeInTheDocument();
  fireEvent.click(notices().getAllByRole('button', { name: 'Fechar aviso: Convite para jogar' })[0]);
  expect(notices().queryByText(/Ranqueada Solo\/Duo/)).not.toBeInTheDocument();
  expect(notices().getByText('Victor convidou você para ARAM')).toBeInTheDocument();
  receive({ type: 'receivedInvitations', invitations: [] });
  expect(region()).not.toBeInTheDocument();
});

it('na seleção de campeões, só avisa mensagens e deixa os convites na coluna', () => {
  render(<App />);
  hello({ gameflowPhase: 'ChampSelect', champSelect: draftChampSelect, conversations: [testConversation] });
  receive({ type: 'receivedInvitations', invitations: [testInvitation] });
  expect(region()).not.toBeInTheDocument();
  chat({ unreadCount: 2, messages: [...testConversation.messages, newMessage] });
  expect(notices().getByText('Mensagem de Amigo#BR1')).toBeInTheDocument();
  expect(notices().queryByText(/convidou você/)).not.toBeInTheDocument();
});

it('avisa partida encontrada até o jogador responder', () => {
  render(<App />);
  hello({ gameflowPhase: 'ReadyCheck', readyCheck: testReadyCheck });
  expect(notices().getByRole('alert')).toHaveTextContent('Partida encontrada!');
  receive({ type: 'readyCheck', readyCheck: { ...testReadyCheck, playerResponse: 'Accepted' } });
  expect(region()).not.toBeInTheDocument();
});

it('limpa os avisos quando a conexão com o agente cai', () => {
  render(<App />);
  hello({ conversations: [testConversation], receivedInvitations: [testInvitation] });
  chat({ unreadCount: 2, messages: [...testConversation.messages, newMessage] });
  act(() => socket().onclose?.());
  expect(region()).not.toBeInTheDocument();
});

it('toca o som uma vez por partida encontrada', () => {
  render(<App />);
  hello({ gameflowPhase: 'Matchmaking' });
  expect(sound.playMatchFound).not.toHaveBeenCalled();
  receive({ type: 'gameflowPhase', phase: 'ReadyCheck' });
  receive({ type: 'readyCheck', readyCheck: testReadyCheck });
  receive({ type: 'readyCheck', readyCheck: { ...testReadyCheck, timer: 3.5 } });
  expect(sound.playMatchFound).toHaveBeenCalledTimes(1);
  receive({ type: 'readyCheck', readyCheck: { ...testReadyCheck, playerResponse: 'Accepted' } });
  receive({ type: 'readyCheck', readyCheck: null });
  receive({ type: 'readyCheck', readyCheck: testReadyCheck });
  expect(sound.playMatchFound).toHaveBeenCalledTimes(2);
});

it('libera o áudio no primeiro clique', () => {
  render(<App />);
  fireEvent.pointerDown(window);
  expect(sound.unlock).toHaveBeenCalledTimes(1);
});

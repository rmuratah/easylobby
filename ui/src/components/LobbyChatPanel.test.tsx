import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { applyEvent, type ConnectionSnapshot } from '../agent/client';
import type { AgentState } from '../bindings/AgentState';
import type { CommandResult } from '../bindings/CommandResult';
import type { LobbyChat } from '../bindings/LobbyChat';
import { testFriend } from '../test/events';
import { testLobby, testState } from '../test/lobby';
import { LobbyChatPanel } from './LobbyChatPanel';

const t = '2026-11-07T20:00:00.000Z';
const chat: LobbyChat = { messages: [
  { id: '1', body: 'bora?', fromMe: false, fromPuuid: 'ana', timestamp: t },
  { id: '2', body: 'bora', fromMe: true, fromPuuid: 'local', timestamp: t },
  { id: '3', body: 'oi', fromMe: false, fromPuuid: 'outro', timestamp: t },
] };
const connected = (overrides: Partial<AgentState>): ConnectionSnapshot => ({ status: 'connected', error: null, state: {
  ...testState, gameflowPhase: 'Lobby', lobby: testLobby, lobbyChat: chat,
  friends: [{ ...testFriend, puuid: 'ana', gameName: 'Ana', tagLine: 'BR1' }], ...overrides } });

describe('chat do grupo', () => {
  it('mostra as mensagens com o nome de quem mandou', () => {
    render(<LobbyChatPanel connection={connected({})} send={vi.fn()} />);
    const log = within(screen.getByRole('log', { name: 'Mensagens do grupo' }));
    expect(log.getAllByRole('listitem').map(item => item.textContent?.replace(/^\S+ /, ''))).toEqual(['Ana#BR1: bora?', 'Você: bora', 'Jogador: oi']);
  });

  it('manda a mensagem e limpa o campo', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'ok' }));
    render(<LobbyChatPanel connection={connected({})} send={send} />);
    const input = screen.getByRole('textbox', { name: 'Mensagem para o grupo' });
    fireEvent.change(input, { target: { value: ' vamos ' } });
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Enviar' })));
    expect(send).toHaveBeenCalledWith({ type: 'sendLobbyMessage', body: 'vamos' });
    expect(input).toHaveValue('');
  });

  it('mostra o erro do agente e mantém o texto', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'lcu', message: 'recusou' }));
    render(<LobbyChatPanel connection={connected({})} send={send} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'oi' } });
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Enviar' })));
    expect(screen.getByRole('alert')).toHaveTextContent('Não foi possível enviar a mensagem: recusou');
    expect(screen.getByRole('textbox')).toHaveValue('oi');
  });

  it('espera a sala abrir e some sem lobby', () => {
    const { rerender } = render(<LobbyChatPanel connection={connected({ lobbyChat: null })} send={vi.fn()} />);
    expect(screen.getByText('Aguardando a sala do grupo abrir…')).toBeInTheDocument();
    rerender(<LobbyChatPanel connection={connected({ lobby: null, lobbyChat: null })} send={vi.fn()} />);
    expect(screen.queryByRole('region', { name: 'Chat do grupo' })).not.toBeInTheDocument();
  });

  it('sair do lobby descarta o chat do grupo', () => {
    const state: AgentState = { ...testState, lcu: { state: 'connected', session: 1 } as AgentState['lcu'], lobby: testLobby, lobbyChat: chat };
    expect(applyEvent(state, { type: 'lobby', lobby: null }).lobbyChat).toBeNull();
    expect(applyEvent({ ...state, lobby: null, lobbyChat: null }, { type: 'lobbyChat', lobbyChat: chat }).lobbyChat).toBeNull();
    expect(applyEvent(state, { type: 'lobbyChat', lobbyChat: null }).lobbyChat).toBeNull();
  });
});

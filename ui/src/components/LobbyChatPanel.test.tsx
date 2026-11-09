import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { applyEvent, type ConnectionSnapshot } from '../agent/client';
import type { AgentState } from '../bindings/AgentState';
import type { CommandResult } from '../bindings/CommandResult';
import type { LobbyChat } from '../bindings/LobbyChat';
import type { LobbyChatMessage } from '../bindings/LobbyChatMessage';
import { testFriend } from '../test/events';
import { testLobby, testState } from '../test/lobby';
import { LobbyChatPanel } from './LobbyChatPanel';

const t = '2026-11-07T20:00:00.000Z';
const line = (m: Partial<LobbyChatMessage> & Pick<LobbyChatMessage, 'id'>): LobbyChatMessage => ({
  kind: 'chat', body: '', fromMe: false, fromPuuid: '', gameName: '', tagLine: '', timestamp: t, ...m });
const chat: LobbyChat = { messages: [
  line({ id: '0', kind: 'joined', fromPuuid: 'kyoto', gameName: 'Kyoto', tagLine: '村田1' }),
  line({ id: '1', body: 'bora?', fromPuuid: 'ana' }),
  line({ id: '2', body: 'bora', fromMe: true, fromPuuid: 'local', gameName: 'Rafinha Lover', tagLine: 'mrt' }),
  line({ id: '3', body: 'oi', fromPuuid: 'outro' }),
  line({ id: '4', kind: 'left', fromPuuid: 'kyoto', gameName: 'Kyoto', tagLine: '村田1' }),
] };
const ana = { ...testLobby.members[0], puuid: 'ana', summonerId: 2, isLeader: false };
const party = { ...testLobby, members: [...testLobby.members, ana] };
const connected = (overrides: Partial<AgentState>): ConnectionSnapshot => ({ status: 'connected', error: null, state: {
  ...testState, gameflowPhase: 'Lobby', lobby: party, lobbyChat: chat,
  friends: [{ ...testFriend, puuid: 'ana', gameName: 'Ana', tagLine: 'BR1' }], ...overrides } });

describe('chat do grupo', () => {
  it('mostra as mensagens e quem entrou e saiu, com o Riot ID de quem mandou', () => {
    render(<LobbyChatPanel connection={connected({})} send={vi.fn()} />);
    const log = within(screen.getByRole('log', { name: 'Mensagens do grupo' }));
    expect(log.getAllByRole('listitem').map(item => item.textContent)).toEqual([
      'Kyoto #村田1 entrou no saguão', 'Ana#BR1: bora?', 'Rafinha Lover #mrt: bora', 'Jogador: oi', 'Kyoto #村田1 saiu do saguão']);
  });

  it('atualiza quando chega mensagem nova', () => {
    const { rerender } = render(<LobbyChatPanel connection={connected({})} send={vi.fn()} />);
    const nova = { messages: [...chat.messages, line({ id: '5', body: 'teste', fromPuuid: 'kyoto', gameName: 'Kyoto', tagLine: '村田1' })] };
    rerender(<LobbyChatPanel connection={connected({ lobbyChat: nova })} send={vi.fn()} />);
    expect(screen.getAllByRole('listitem').at(-1)).toHaveTextContent('Kyoto #村田1: teste');
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

  it('some sozinho no lobby e volta quando alguém entra (#285)', () => {
    const { rerender } = render(<LobbyChatPanel connection={connected({ lobby: testLobby })} send={vi.fn()} />);
    expect(screen.queryByRole('region', { name: 'Chat do grupo' })).not.toBeInTheDocument();
    rerender(<LobbyChatPanel connection={connected({ lobby: party })} send={vi.fn()} />);
    expect(screen.getByRole('region', { name: 'Chat do grupo' })).toBeInTheDocument();
  });

  it('sair do lobby descarta o chat do grupo', () => {
    const state: AgentState = { ...testState, lcu: { state: 'connected', session: 1 } as AgentState['lcu'], lobby: testLobby, lobbyChat: chat };
    expect(applyEvent(state, { type: 'lobby', lobby: null }).lobbyChat).toBeNull();
    expect(applyEvent({ ...state, lobby: null, lobbyChat: null }, { type: 'lobbyChat', lobbyChat: chat }).lobbyChat).toBeNull();
    expect(applyEvent(state, { type: 'lobbyChat', lobbyChat: null }).lobbyChat).toBeNull();
  });
});

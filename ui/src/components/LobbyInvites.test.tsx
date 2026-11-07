import { act, createEvent, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { FRIEND_DRAG_TYPE } from '../agent/lobbyInvite';
import type { AgentState } from '../bindings/AgentState';
import type { CommandResult } from '../bindings/CommandResult';
import type { Friend } from '../bindings/Friend';
import type { Lobby } from '../bindings/Lobby';
import { testFriend } from '../test/events';
import { testLobby, testState } from '../test/lobby';
import { LobbyGroupCard } from './LobbyInvites';
import { LobbyPanel } from './LobbyPanel';

const friend = (overrides: Partial<Friend>): Friend => ({ ...testFriend, ...overrides });
const ana = friend({ id: 'ana', puuid: 'ana', summonerId: 2001, gameName: 'Ana', tagLine: 'BR1' });
const bia = friend({ id: 'bia', puuid: 'bia', summonerId: 2002, gameName: 'Bia', tagLine: 'BR1' });
const caio = friend({ id: 'caio', puuid: 'caio', summonerId: 2003, gameName: 'Caio', tagLine: 'BR1', availability: 'offline' });
const flexLobby: Lobby = { ...testLobby, queueId: 440, queue: 'rankedFlex', maxLobbySize: 5 };

const connected = (overrides: Partial<AgentState>): ConnectionSnapshot =>
  ({ status: 'connected', error: null, state: { ...testState, gameflowPhase: 'Lobby', friends: [ana, bia, caio], lobby: flexLobby, ...overrides } });
const ok = () => vi.fn(async (): Promise<CommandResult> => ({ status: 'ok' }));
const lobby = () => within(screen.getByRole('region', { name: 'Lobby' }));

/** Solta no lobby um amigo arrastado da lista de amigos. */
function drop(puuid: string) {
  const target = screen.getByRole('region', { name: 'Lobby' });
  const dataTransfer = { types: [FRIEND_DRAG_TYPE], getData: (type: string) => (type === FRIEND_DRAG_TYPE ? puuid : ''), dropEffect: 'none' };
  fireEvent(target, Object.assign(createEvent.dragOver(target), { dataTransfer }));
  return act(async () => { fireEvent(target, Object.assign(createEvent.drop(target), { dataTransfer })); });
}

describe('convidar pelo lobby', () => {
  it('o "+" da vaga livre abre os amigos disponíveis e convida', async () => {
    const send = ok();
    const { rerender } = render(<LobbyPanel connection={connected({})} send={send} />);
    const plus = lobby().getAllByRole('button', { name: 'Convidar amigo' });
    expect(plus).toHaveLength(4);
    fireEvent.click(plus[0]);
    const picker = within(screen.getByRole('dialog', { name: 'Convidar amigos' }));
    expect(picker.getAllByRole('listitem').map(item => item.textContent)).toEqual(['ANAna#BR1Convidar', 'BIBia#BR1Convidar']);
    await act(async () => fireEvent.click(picker.getByRole('button', { name: 'Convidar Ana#BR1' })));
    expect(send).toHaveBeenCalledWith({ type: 'inviteToLobby', invitees: [{ summonerId: 2001, puuid: 'ana' }] });
    // Até o convite aparecer no lobby, Ana não volta para a lista.
    expect(picker.queryByRole('button', { name: 'Convidar Ana#BR1' })).not.toBeInTheDocument();
    rerender(<LobbyPanel connection={connected({ lobby: { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 2001, state: 'Declined' }] } })} send={send} />);
    expect(within(screen.getByRole('dialog')).getByRole('button', { name: 'Convidar Ana#BR1' })).toBeEnabled();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('sem permissão de convidar, a vaga livre não tem botão', () => {
    render(<LobbyPanel connection={connected({ lobby: { ...flexLobby, isLeader: false, canInvite: false } })} send={vi.fn()} />);
    expect(lobby().queryByRole('button', { name: 'Convidar amigo' })).not.toBeInTheDocument();
    expect(lobby().getAllByRole('listitem', { name: 'Vaga livre' })).toHaveLength(4);
  });

  it('arrastar um amigo para o lobby convida', async () => {
    const send = ok();
    render(<LobbyPanel connection={connected({})} send={send} />);
    await drop('bia');
    expect(send).toHaveBeenCalledWith({ type: 'inviteToLobby', invitees: [{ summonerId: 2002, puuid: 'bia' }] });
  });

  it('arrastar quem não pode ser convidado explica o motivo e não manda nada', async () => {
    const send = ok();
    const { rerender } = render(<LobbyPanel connection={connected({})} send={send} />);
    await drop('caio');
    expect(lobby().getByRole('alert')).toHaveTextContent('Caio#BR1 não pode ser convidado agora');
    rerender(<LobbyPanel connection={connected({ lobby: { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 2001, state: 'Pending' }] } })} send={send} />);
    await drop('ana');
    expect(lobby().getByRole('alert')).toHaveTextContent('Ana#BR1 já foi convidado.');
    rerender(<LobbyPanel connection={connected({ gameflowPhase: 'Matchmaking' })} send={send} />);
    await drop('bia');
    expect(lobby().getByRole('alert')).toHaveTextContent('Não dá para convidar agora');
    expect(send).not.toHaveBeenCalled();
  });

  it('mostra o erro do agente ao convidar', async () => {
    const send: AgentClient['send'] = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'lcu', message: 'falhou' }));
    render(<LobbyPanel connection={connected({})} send={send} />);
    await drop('bia');
    expect(lobby().getByRole('alert')).toHaveTextContent('Não foi possível convidar Bia#BR1: falhou');
  });
});

describe('card do grupo', () => {
  it('lista quem entrou e o status de cada convite', () => {
    const member = { ...testLobby.members[0], isLeader: false };
    render(<LobbyGroupCard connection={connected({ lobby: { ...flexLobby,
      members: [testLobby.members[0], { ...member, puuid: 'ana', summonerId: 2001 }],
      invitations: [
        { puuid: 'ana', summonerId: 2001, state: 'Accepted' },
        { puuid: 'bia', summonerId: 2002, state: 'Pending' },
        { puuid: 'caio', summonerId: 2003, state: 'Declined' },
      ] } })} />);
    expect(screen.getByRole('heading', { name: 'Grupo (3)' })).toBeInTheDocument();
    expect(within(screen.getByRole('list', { name: 'Convidados' })).getAllByRole('listitem').map(item => item.textContent))
      .toEqual(['Ana#BR1No grupo', '…Bia#BR1Convite pendente', 'Caio#BR1Recusou o convite']);
  });

  it('sem convidados, explica como convidar; sem lobby, some', () => {
    const { rerender } = render(<LobbyGroupCard connection={connected({})} />);
    expect(screen.getByRole('heading', { name: 'Grupo (0)' })).toBeInTheDocument();
    expect(screen.getByText(/arraste um amigo/)).toBeInTheDocument();
    rerender(<LobbyGroupCard connection={connected({ lobby: null })} />);
    expect(screen.queryByRole('region', { name: 'Grupo' })).not.toBeInTheDocument();
  });
});

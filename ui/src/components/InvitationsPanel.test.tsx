import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import { freeSlots, invitableFriends } from '../agent/invitations';
import type { AgentState } from '../bindings/AgentState';
import type { CommandResult } from '../bindings/CommandResult';
import type { Friend } from '../bindings/Friend';
import type { Lobby } from '../bindings/Lobby';
import { testFriend, testInvitation } from '../test/events';
import { testLobby, testState } from '../test/lobby';
import { InvitationsPanel } from './InvitationsPanel';

const friend = (overrides: Partial<Friend>): Friend => ({ ...testFriend, ...overrides });
const ana = friend({ id: 'ana', puuid: 'ana', summonerId: 2001, gameName: 'Ana', tagLine: 'BR1' });
const bia = friend({ id: 'bia', puuid: 'bia', gameName: 'Bia', tagLine: 'BR1' });
const caio = friend({ id: 'caio', puuid: 'caio', gameName: 'Caio', tagLine: 'BR1', availability: 'offline' });
const davi = friend({ id: 'davi', puuid: 'davi', gameName: 'Davi', tagLine: 'BR1', availability: 'mobile' });
const flexLobby: Lobby = { ...testLobby, queueId: 440, queue: 'rankedFlex', maxLobbySize: 5 };

const connected = (overrides: Partial<AgentState>): ConnectionSnapshot =>
  ({ status: 'connected', error: null, state: { ...testState, gameflowPhase: 'Lobby', friends: [ana, bia, caio, davi], ...overrides } });

function deferred() {
  let resolve!: (result: CommandResult) => void;
  const promise = new Promise<CommandResult>(r => { resolve = r; });
  return { promise, resolve };
}

describe('regras de convite', () => {
  it('desconta membros e convites em aberto das vagas', () => {
    const lobby: Lobby = { ...flexLobby, invitations: [
      { puuid: 'ana', summonerId: 0, state: 'Pending' },
      { puuid: 'bia', summonerId: 0, state: 'Declined' },
    ] };
    expect(freeSlots(lobby)).toBe(3);
    expect(freeSlots({ ...testLobby, invitations: [{ puuid: 'ana', summonerId: 0, state: 'Pending' }] })).toBe(0);
  });

  it('oferece só amigos online no PC, fora do grupo e sem convite em aberto', () => {
    const lobby: Lobby = { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 0, state: 'Pending' }] };
    expect(invitableFriends(lobby, [ana, bia, caio, davi]).map(f => f.puuid)).toEqual(['bia']);
    const declined: Lobby = { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 0, state: 'Declined' }] };
    expect(invitableFriends(declined, [ana, bia]).map(f => f.puuid)).toEqual(['ana', 'bia']);
    const semLol = friend({ id: 'eva', puuid: 'eva', gameName: 'Eva', summonerId: 0 });
    expect(invitableFriends(flexLobby, [semLol, bia]).map(f => f.puuid)).toEqual(['bia']);
  });
});

describe('InvitationsPanel', () => {
  it('pede para aguardar sem conexão com o LoL', () => {
    render(<InvitationsPanel connection={{ status: 'reconnecting', error: null, state: null }} send={vi.fn()} />);
    expect(screen.getByRole('region', { name: 'Convites' })).toHaveTextContent('Aguarde a conexão');
  });

  it('aceita um convite recebido uma vez só e espera o evento', async () => {
    const reply = deferred();
    const send = vi.fn(() => reply.promise);
    const { rerender } = render(<InvitationsPanel connection={connected({ gameflowPhase: 'None', receivedInvitations: [testInvitation] })} send={send} />);
    const list = within(screen.getByRole('list', { name: 'Convites recebidos' }));
    expect(list.getByRole('listitem')).toHaveTextContent('Um amigo chamou você para Ranqueada Solo/Duo.');
    const accept = list.getByRole('button', { name: 'Aceitar convite de Um amigo' });
    act(() => { fireEvent.click(accept); fireEvent.click(accept); });
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith({ type: 'acceptInvitation', invitationId: 'convite-1' });
    await act(async () => reply.resolve({ status: 'ok' }));
    expect(screen.getByRole('status')).toHaveTextContent('Convite de Um amigo aceito.');
    expect(accept).toBeDisabled();
    rerender(<InvitationsPanel connection={connected({ gameflowPhase: 'None', receivedInvitations: [] })} send={send} />);
    expect(screen.getByText('Nenhum convite recebido.')).toBeInTheDocument();
  });

  it('recusa e mostra o erro do agente', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'lcu', message: 'convite expirou' }));
    const invitation = { ...testInvitation, fromSummonerName: 'Bia', queue: null, queueId: 1700 };
    render(<InvitationsPanel connection={connected({ receivedInvitations: [invitation] })} send={send} />);
    expect(screen.getByRole('listitem')).toHaveTextContent('Bia chamou você para outra fila (ID 1700).');
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Recusar convite de Bia' })));
    expect(send).toHaveBeenCalledWith({ type: 'declineInvitation', invitationId: 'convite-1' });
    expect(screen.getByRole('alert')).toHaveTextContent('Não foi possível concluir: convite expirou');
    expect(screen.getByRole('button', { name: 'Recusar convite de Bia' })).toBeEnabled();
  });

  it('não deixa aceitar quando a LCU não permite', () => {
    render(<InvitationsPanel connection={connected({ receivedInvitations: [{ ...testInvitation, canAccept: false }] })} send={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Aceitar convite de Um amigo' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Recusar convite de Um amigo' })).toBeEnabled();
    expect(screen.getByText(/Não dá para aceitar agora/)).toBeInTheDocument();
  });

  it('pede um lobby antes de convidar', () => {
    render(<InvitationsPanel connection={connected({ lobby: null, gameflowPhase: 'None' })} send={vi.fn()} />);
    expect(screen.getByText('Entre em um lobby para convidar amigos.')).toBeInTheDocument();
  });

  it('convida um amigo pelo puuid e some com ele até o convite chegar', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'ok' }));
    const { rerender } = render(<InvitationsPanel connection={connected({ lobby: flexLobby })} send={send} />);
    const candidates = within(screen.getByRole('list', { name: 'Amigos para convidar' }));
    expect(candidates.getAllByRole('listitem').map(item => item.textContent)).toEqual(['Ana#BR1Convidar', 'Bia#BR1Convidar']);
    expect(screen.getByText('4 vagas livres no grupo.')).toBeInTheDocument();
    await act(async () => fireEvent.click(candidates.getByRole('button', { name: 'Convidar Ana#BR1' })));
    expect(send).toHaveBeenCalledWith({ type: 'inviteToLobby', invitees: [{ summonerId: 2001, puuid: 'ana' }] });
    expect(screen.getByRole('status')).toHaveTextContent('Convite enviado para Ana#BR1.');
    expect(screen.queryByRole('button', { name: 'Convidar Ana#BR1' })).not.toBeInTheDocument();
    expect(screen.getByText('3 vagas livres no grupo.')).toBeInTheDocument();

    const pending: Lobby = { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 0, state: 'Pending' }] };
    rerender(<InvitationsPanel connection={connected({ lobby: pending })} send={send} />);
    const sent = within(screen.getByRole('list', { name: 'Convites enviados' }));
    expect(sent.getByRole('listitem')).toHaveTextContent('Ana#BR1Aguardando resposta');
    expect(screen.getByText('3 vagas livres no grupo.')).toBeInTheDocument();

    const declined: Lobby = { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 0, state: 'Declined' }] };
    rerender(<InvitationsPanel connection={connected({ lobby: declined })} send={send} />);
    expect(screen.getByRole('list', { name: 'Convites enviados' })).toHaveTextContent('Ana#BR1Recusou');
    expect(screen.getByRole('button', { name: 'Convidar Ana#BR1' })).toBeEnabled();
  });

  it('bloqueia convite sem permissão, com grupo cheio ou fora da fase de lobby', () => {
    const send = vi.fn();
    const { rerender } = render(<InvitationsPanel connection={connected({ lobby: { ...flexLobby, isLeader: false, canInvite: false } })} send={send} />);
    expect(screen.getByText('Só o líder pode convidar neste lobby.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Convidar Ana#BR1' })).toBeDisabled();

    rerender(<InvitationsPanel connection={connected({ lobby: { ...testLobby, invitations: [{ puuid: 'bia', summonerId: 0, state: 'Pending' }] } })} send={send} />);
    expect(screen.getByText('O grupo está cheio.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Convidar Ana#BR1' })).toBeDisabled();

    rerender(<InvitationsPanel connection={connected({ lobby: flexLobby, gameflowPhase: 'Matchmaking' })} send={send} />);
    expect(screen.getByText(/fase de lobby/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Convidar Ana#BR1' })).toBeDisabled();
    expect(send).not.toHaveBeenCalled();
  });

  it('mostra quem não está na lista de amigos', () => {
    const lobby: Lobby = { ...flexLobby, invitations: [{ puuid: 'estranho', summonerId: 9, state: 'Kicked' }] };
    render(<InvitationsPanel connection={connected({ lobby })} send={vi.fn()} />);
    expect(screen.getByRole('list', { name: 'Convites enviados' })).toHaveTextContent('Jogador fora da lista de amigosRemovido do grupo');
  });
});

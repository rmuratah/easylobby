import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
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

const panel = (connection: ConnectionSnapshot, send: AgentClient['send']) => <InvitationsPanel connection={connection} send={send} />;

describe('InvitationsPanel', () => {
  it('sem conexão com o LoL, não mostra seção de convites', () => {
    render(panel({ status: 'reconnecting', error: null, state: null }, vi.fn()));
    expect(screen.queryByRole('region', { name: 'Convites recebidos' })).not.toBeInTheDocument();
  });

  it('não mostra seção de convites sem convite recebido', () => {
    render(panel(connected({ gameflowPhase: 'None' }), vi.fn()));
    expect(screen.queryByRole('region', { name: 'Convites recebidos' })).not.toBeInTheDocument();
  });

  it('aceita um convite recebido uma vez só e espera o evento', async () => {
    const reply = deferred();
    const send = vi.fn(() => reply.promise);
    const { rerender } = render(panel(connected({ gameflowPhase: 'None', receivedInvitations: [testInvitation] }), send));
    const list = within(screen.getByRole('list', { name: 'Convites recebidos' }));
    expect(list.getByRole('listitem')).toHaveTextContent('Um amigo chamou você para Ranqueada Solo/Duo.');
    const accept = list.getByRole('button', { name: 'Aceitar convite de Um amigo' });
    act(() => { fireEvent.click(accept); fireEvent.click(accept); });
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith({ type: 'acceptInvitation', invitationId: 'convite-1' });
    await act(async () => reply.resolve({ status: 'ok' }));
    expect(screen.getByRole('status')).toHaveTextContent('Convite de Um amigo aceito.');
    expect(accept).toBeDisabled();
    rerender(panel(connected({ gameflowPhase: 'None', receivedInvitations: [] }), send));
    expect(screen.queryByRole('region', { name: 'Convites recebidos' })).not.toBeInTheDocument();
  });

  it('recusa e mostra o erro do agente', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'lcu', message: 'convite expirou' }));
    const invitation = { ...testInvitation, fromSummonerName: 'Bia', queue: null, queueId: 1700 };
    render(panel(connected({ receivedInvitations: [invitation] }), send));
    const list = within(screen.getByRole('list', { name: 'Convites recebidos' }));
    expect(list.getByRole('listitem')).toHaveTextContent('Bia chamou você para outra fila (ID 1700).');
    await act(async () => fireEvent.click(screen.getByRole('button', { name: 'Recusar convite de Bia' })));
    expect(send).toHaveBeenCalledWith({ type: 'declineInvitation', invitationId: 'convite-1' });
    expect(screen.getByRole('alert')).toHaveTextContent('Não foi possível concluir: convite expirou');
    expect(screen.getByRole('button', { name: 'Recusar convite de Bia' })).toBeEnabled();
  });

  it('mostra o nick do amigo que convidou e o nome da fila da LCU', () => {
    const desordem = {
      queueId: 2400, queue: null, name: 'ARAM: Desordem', description: 'ARAM: Desordem', gameMode: 'KIWI', modeGroup: '',
      mapId: 12, versusAi: false, ranked: false, showPositionSelector: false, limitedTime: true, available: true,
    };
    const invitation = { ...testInvitation, fromSummonerId: 2001, fromSummonerName: '', queue: null, queueId: 2400 };
    render(panel(connected({ receivedInvitations: [invitation], queues: [desordem] }), vi.fn()));
    const item = within(screen.getByRole('list', { name: 'Convites recebidos' })).getByRole('listitem');
    expect(item).toHaveTextContent('Ana chamou você para ARAM: Desordem.');
    expect(within(item).getByText('Ana')).toHaveAttribute('title', 'Ana#BR1');
    expect(within(item).getByRole('button', { name: 'Aceitar convite de Ana' })).toBeEnabled();
  });

  it('usa o nome fixo da fila conhecida e o nome da LCU quando quem convidou não é amigo', () => {
    const invitation = { ...testInvitation, fromSummonerId: 9999, fromSummonerName: 'Zed', queue: 'aram' as const, queueId: 450 };
    render(panel(connected({ receivedInvitations: [invitation] }), vi.fn()));
    expect(within(screen.getByRole('list', { name: 'Convites recebidos' })).getByRole('listitem'))
      .toHaveTextContent('Zed chamou você para ARAM.');
  });

  it('não deixa aceitar quando a LCU não permite', () => {
    render(panel(connected({ receivedInvitations: [{ ...testInvitation, canAccept: false }] }), vi.fn()));
    expect(screen.getByRole('button', { name: 'Aceitar convite de Um amigo' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Recusar convite de Um amigo' })).toBeEnabled();
    expect(screen.getByText(/Não dá para aceitar agora/)).toBeInTheDocument();
  });
});

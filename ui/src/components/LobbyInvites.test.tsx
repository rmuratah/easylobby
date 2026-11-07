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
  it('o "+" da vaga livre abre o pop-up e convida os amigos marcados', async () => {
    const send = ok();
    const { rerender } = render(<LobbyPanel connection={connected({})} send={send} />);
    const plus = lobby().getAllByRole('button', { name: 'Convidar amigo' });
    expect(plus).toHaveLength(4);
    fireEvent.click(plus[0]);
    const picker = within(screen.getByRole('dialog', { name: 'Convidar amigos' }));
    // Caio está offline: só Ana e Bia aparecem, na pasta padrão.
    expect(picker.getAllByRole('checkbox').map(box => box.getAttribute('aria-label'))).toEqual(['Convidar Ana#BR1', 'Convidar Bia#BR1']);
    expect(picker.getByRole('button', { name: 'Enviar convites' })).toBeDisabled();
    fireEvent.click(picker.getByRole('checkbox', { name: 'Convidar Ana#BR1' }));
    fireEvent.click(picker.getByRole('checkbox', { name: 'Convidar Bia#BR1' }));
    await act(async () => fireEvent.click(picker.getByRole('button', { name: 'Enviar convites' })));
    expect(send).toHaveBeenCalledWith({ type: 'inviteToLobby', invitees: [{ summonerId: 2001, puuid: 'ana' }, { summonerId: 2002, puuid: 'bia' }] });
    // Convites aceitos fecham o pop-up.
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    // Até o convite aparecer no lobby, Ana não volta para a lista; recusado, ela volta.
    rerender(<LobbyPanel connection={connected({ lobby: { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 2001, state: 'Declined' }] } })} send={send} />);
    fireEvent.click(lobby().getAllByRole('button', { name: 'Convidar amigo' })[0]);
    expect(within(screen.getByRole('dialog')).getByRole('checkbox', { name: 'Convidar Ana#BR1' })).toBeEnabled();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('filtra pelo nome de jogo e pela tagline, e respeita as vagas livres', () => {
    const duoLobby: Lobby = { ...flexLobby, maxLobbySize: 2 };
    render(<LobbyPanel connection={connected({ lobby: duoLobby, friends: [ana, bia, friend({ id: 'bea', puuid: 'bea', summonerId: 2004, gameName: 'Béa', tagLine: 'XYZ' })] })} send={ok()} />);
    fireEvent.click(lobby().getByRole('button', { name: 'Convidar amigo' }));
    const picker = within(screen.getByRole('dialog', { name: 'Convidar amigos' }));
    fireEvent.change(picker.getByLabelText('Nome de jogo'), { target: { value: 'be' } });
    expect(picker.getAllByRole('checkbox').map(box => box.getAttribute('aria-label'))).toEqual(['Convidar Béa#XYZ']);
    fireEvent.change(picker.getByLabelText('Tagline'), { target: { value: 'br' } });
    expect(picker.getByText('Nenhum amigo disponível com esse nome.')).toBeInTheDocument();
    fireEvent.click(picker.getByRole('button', { name: 'Limpar nome de jogo' }));
    fireEvent.click(picker.getByRole('button', { name: 'Limpar tagline' }));
    // Uma vaga livre: marcado um, os outros ficam bloqueados.
    fireEvent.click(picker.getByRole('checkbox', { name: 'Convidar Ana#BR1' }));
    expect(picker.getByRole('checkbox', { name: 'Convidar Bia#BR1' })).toBeDisabled();
  });

  it('convida pelo Riot ID digitado no filtro', async () => {
    const send = ok();
    render(<LobbyPanel connection={connected({})} send={send} />);
    fireEvent.click(lobby().getAllByRole('button', { name: 'Convidar amigo' })[0]);
    const picker = within(screen.getByRole('dialog', { name: 'Convidar amigos' }));
    fireEvent.change(picker.getByLabelText('Nome de jogo'), { target: { value: 'árabe' } });
    fireEvent.change(picker.getByLabelText('Tagline'), { target: { value: '#arabi' } });
    await act(async () => fireEvent.click(picker.getByRole('button', { name: 'Convidar árabe#arabi pelo nome' })));
    expect(send).toHaveBeenCalledWith({ type: 'inviteByRiotId', gameName: 'árabe', tagLine: '#arabi' });
    expect(picker.getByText('Convite enviado')).toBeInTheDocument();
  });

  it('jogador não encontrado pelo nome explica', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'notFound', message: 'x' }));
    render(<LobbyPanel connection={connected({})} send={send} />);
    fireEvent.click(lobby().getAllByRole('button', { name: 'Convidar amigo' })[0]);
    const picker = within(screen.getByRole('dialog', { name: 'Convidar amigos' }));
    fireEvent.change(picker.getByLabelText('Nome de jogo'), { target: { value: 'Zé' } });
    fireEvent.change(picker.getByLabelText('Tagline'), { target: { value: 'BR1' } });
    await act(async () => fireEvent.click(picker.getByRole('button', { name: 'Convidar Zé#BR1 pelo nome' })));
    expect(picker.getByRole('alert')).toHaveTextContent('Nenhum jogador encontrado com o Riot ID Zé#BR1.');
  });

  it('pede os recentes ao abrir e convida um deles', async () => {
    const send = ok();
    const recentPlayers = [
      { puuid: 'rafa', summonerId: 3001, gameName: 'Rafinha', tagLine: 'BR1', summonerName: '', championId: 64, playedAt: '2026-11-07T20:00:00.000Z' },
      { puuid: 'ana', summonerId: 2001, gameName: 'Ana', tagLine: 'BR1', summonerName: '', championId: null, playedAt: '2026-11-07T19:00:00.000Z' },
    ];
    render(<LobbyPanel connection={connected({ recentPlayers, lobby: { ...flexLobby, invitations: [{ puuid: 'ana', summonerId: 2001, state: 'Pending' }] } })} send={send} />);
    fireEvent.click(lobby().getAllByRole('button', { name: 'Convidar amigo' })[0]);
    await act(async () => {});
    expect(send).toHaveBeenCalledWith({ type: 'loadRecentPlayers' });
    const picker = within(screen.getByRole('dialog', { name: 'Convidar amigos' }));
    const recentes = picker.getByRole('button', { name: /Recentes/ });
    expect(recentes).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(recentes);
    // Ana já foi convidada: fica de fora.
    expect(within(picker.getByRole('region', { name: 'Recentes' })).getAllByRole('checkbox').map(b => b.getAttribute('aria-label')))
      .toEqual(['Convidar Rafinha#BR1']);
    fireEvent.click(picker.getByRole('checkbox', { name: 'Convidar Rafinha#BR1' }));
    await act(async () => fireEvent.click(picker.getByRole('button', { name: 'Enviar convites' })));
    expect(send).toHaveBeenCalledWith({ type: 'inviteToLobby', invitees: [{ summonerId: 3001, puuid: 'rafa' }] });
  });

  it('as pastas de amigos abrem e fecham', () => {
    const amigos = { id: 7, name: 'Fiotes', priority: 1, collapsed: true };
    render(<LobbyPanel connection={connected({ friends: [ana, { ...bia, groupId: 7, groupName: 'Fiotes' }], friendGroups: [{ id: 0, name: '**Default', priority: 0, collapsed: false }, amigos] })} send={ok()} />);
    fireEvent.click(lobby().getAllByRole('button', { name: 'Convidar amigo' })[0]);
    const picker = within(screen.getByRole('dialog', { name: 'Convidar amigos' }));
    const fiotes = picker.getByRole('button', { name: /Fiotes/ });
    expect(fiotes).toHaveAttribute('aria-expanded', 'false');
    expect(picker.queryByRole('checkbox', { name: 'Convidar Bia#BR1' })).not.toBeInTheDocument();
    fireEvent.click(fiotes);
    expect(picker.getByRole('checkbox', { name: 'Convidar Bia#BR1' })).toBeInTheDocument();
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

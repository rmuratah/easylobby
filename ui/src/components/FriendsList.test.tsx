import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import { describeActivity, friendDisplayName, friendGroupName, friendNick, groupFriends } from '../agent/friends';
import type { Friend } from '../bindings/Friend';
import type { FriendGroup } from '../bindings/FriendGroup';
import { testFriend } from '../test/events';
import { makeState } from '../test/state';
import { FriendsList } from './FriendsList';

const NOW = Date.UTC(2026, 10, 6, 12, 0, 0);
const friend = (overrides: Partial<Friend>): Friend => ({ ...testFriend, ...overrides });
const connectedWith = (friends: Friend[], friendGroups: FriendGroup[] = []): ConnectionSnapshot =>
  ({ status: 'connected', error: null, state: makeState({ friends, friendGroups }) });

const ana = friend({ id: 'ana', gameName: 'Ana', tagLine: 'BR1', profileIconId: 29,
  activity: { type: 'league', status: 'inGame', queueId: 420, championId: 266, since: NOW - 12 * 60_000 } });
const bia = friend({ id: 'bia', gameName: 'Bia', tagLine: 'BR1', availability: 'away', statusMessage: 'volto já' });
const caio = friend({ id: 'caio', gameName: '', tagLine: '', name: 'CaioAntigo', availability: 'offline' });
const duda = friend({ id: 'duda', gameName: 'Duda', tagLine: 'DUO', groupId: 2, groupName: 'duo' });
const edu = friend({ id: 'edu', gameName: 'Edu', tagLine: 'BR1', groupId: 2, groupName: 'duo', availability: 'offline' });
const group = (id: number, name: string, priority: number, collapsed = false): FriendGroup => ({ id, name, priority, collapsed });

afterEach(() => { vi.useRealTimers(); });

describe('FriendsList', () => {
  it('pede para aguardar sem conexão com o LoL', () => {
    render(<FriendsList connection={{ status: 'reconnecting', error: null, state: null }} />);
    expect(screen.getByRole('region', { name: 'Amigos' })).toHaveTextContent('Aguarde a conexão');
  });

  it('avisa quando nenhum amigo está online', () => {
    render(<FriendsList connection={connectedWith([caio])} />);
    expect(screen.getByText('Nenhum amigo online agora.')).toBeInTheDocument();
    expect(screen.queryByRole('list', { name: 'Amigos online' })).toBeNull();
  });

  it('avisa quando a lista está vazia', () => {
    render(<FriendsList connection={connectedWith([])} />);
    expect(screen.getByText('Você ainda não tem amigos na lista.')).toBeInTheDocument();
  });

  it('separa online e offline, mantendo a ordem do agente, e mostra só o nick', () => {
    vi.useFakeTimers({ now: NOW });
    render(<FriendsList connection={connectedWith([ana, bia, caio])} />);
    expect(screen.getByRole('heading', { name: /Amigos/ })).toHaveTextContent('(2 de 3 online)');
    const online = within(screen.getByRole('list', { name: 'Amigos online em Geral' })).getAllByRole('listitem');
    expect(online.map(item => item.textContent)).toEqual([
      expect.stringContaining('Ana'), expect.stringContaining('Bia'),
    ]);
    expect(screen.queryByText(/#BR1/)).toBeNull();
    expect(within(online[0]).getByText('Ana')).toHaveAttribute('title', 'Ana#BR1');
    expect(online[0]).toHaveTextContent('Em partida · Ranqueada Solo/Duo · 12 min');
    expect(within(online[0]).getByRole('img', { name: 'Online' })).toBeInTheDocument();
    expect(within(online[0]).queryByRole('img', { name: 'Campeão da partida' })).toBeNull();
    expect(online[0].querySelector('img[src*="/assets/champion/"]')).toBeNull();
    expect(online[1]).toHaveTextContent('Ausente');
    expect(online[1]).toHaveTextContent('volto já');
    const offline = within(screen.getByRole('list', { name: 'Amigos offline', hidden: true })).getAllByRole('listitem', { hidden: true });
    expect(offline).toHaveLength(1);
    expect(offline[0]).toHaveTextContent('CaioAntigo');
    expect(screen.getByRole('button', { name: 'Offline (1)' })).toHaveAttribute('aria-expanded', 'false');
  });

  it('agrupa os online nas pastas do client oficial, na ordem dele', () => {
    render(<FriendsList connection={connectedWith([ana, duda, caio, edu], [group(2, 'duo', -1), group(0, '**Default', 0)])} />);
    const folders = screen.getAllByRole('button', { expanded: true }).map(button => button.textContent);
    expect(folders).toEqual(['duo (1/2)', 'Geral (1/2)']);
    expect(screen.getByRole('list', { name: 'Amigos online em duo' })).toHaveTextContent('Duda');
    expect(screen.getByRole('list', { name: 'Amigos online em Geral' })).toHaveTextContent('Ana');
    expect(screen.getByRole('list', { name: 'Amigos offline', hidden: true })).toHaveTextContent(/CaioAntigo.*Edu/);
  });

  it('abre e fecha cada pasta, começando como o client oficial deixou', () => {
    render(<FriendsList connection={connectedWith([ana, duda], [group(0, '**Default', 0), group(2, 'duo', 1, true)])} />);
    const duo = screen.getByRole('button', { name: 'duo (1/1)' });
    expect(duo).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('list', { name: 'Amigos online em duo' })).toBeNull();
    fireEvent.click(duo);
    expect(duo).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('list', { name: 'Amigos online em duo' })).toHaveTextContent('Duda');
    const geral = screen.getByRole('button', { name: 'Geral (1/1)' });
    fireEvent.click(geral);
    expect(screen.queryByRole('list', { name: 'Amigos online em Geral' })).toBeNull();
    // Buscar abre tudo, para achar quem está numa pasta fechada.
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar amigo' }), { target: { value: 'ana' } });
    expect(screen.getByRole('list', { name: 'Amigos online em Geral' })).toHaveTextContent('Ana');
  });

  it('acha o amigo pela tag, mesmo sem mostrá-la', () => {
    render(<FriendsList connection={connectedWith([ana, duda])} />);
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar amigo' }), { target: { value: '#duo' } });
    expect(screen.getByRole('list', { name: 'Amigos online em duo' })).toHaveTextContent('Duda');
    expect(screen.queryByRole('list', { name: 'Amigos online em Geral' })).toBeNull();
  });

  it('atualiza o tempo da atividade com o relógio', () => {
    vi.useFakeTimers({ now: NOW });
    render(<FriendsList connection={connectedWith([ana])} />);
    expect(screen.getByText(/12 min/)).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(60_000); });
    expect(screen.getByText(/13 min/)).toBeInTheDocument();
  });

  it('filtra pelo nome', () => {
    render(<FriendsList connection={connectedWith([ana, bia, caio])} />);
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar amigo' }), { target: { value: 'caio' } });
    expect(screen.queryByRole('list', { name: /Amigos online/ })).not.toBeInTheDocument();
    expect(screen.getByRole('list', { name: 'Amigos offline' })).toHaveTextContent('CaioAntigo');
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar amigo' }), { target: { value: 'zzz' } });
    expect(screen.getByText('Nenhum amigo encontrado.')).toBeInTheDocument();
  });

  it('mostra as ações de cada amigo vindas de fora', () => {
    render(<FriendsList connection={connectedWith([ana, bia])}
      renderActions={f => <button type="button">Conversar com {f.gameName}</button>} />);
    expect(screen.getByRole('button', { name: 'Conversar com Ana' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Conversar com Bia' })).toBeInTheDocument();
  });

  it('troca o ícone que não carrega por um quadrado vazio', () => {
    const { container } = render(<FriendsList connection={connectedWith([ana])} />);
    const icon = container.querySelector('img[src$="/assets/profileIcon/29"]')!;
    fireEvent.error(icon);
    expect(container.querySelector('img[src$="/assets/profileIcon/29"]')).toBeNull();
  });
});

describe('textos dos amigos', () => {
  it('usa o nome antigo sem Riot ID', () => {
    expect(friendDisplayName(caio)).toBe('CaioAntigo');
    expect(friendDisplayName(friend({ gameName: '', name: '' }))).toBe('Amigo');
    expect(friendNick(caio)).toBe('CaioAntigo');
    expect(friendNick(friend({ gameName: '', name: '' }))).toBe('Amigo');
  });

  it('mostra o nick sem a tag e o Riot ID completo à parte', () => {
    expect(friendNick(ana)).toBe('Ana');
    expect(friendDisplayName(ana)).toBe('Ana#BR1');
  });

  it('traduz o nome da pasta padrão', () => {
    expect(friendGroupName('**Default')).toBe('Geral');
    expect(friendGroupName('')).toBe('Geral');
    expect(friendGroupName('duo')).toBe('duo');
  });

  it('cria as pastas que a LCU não listou a partir dos amigos', () => {
    const solto = friend({ id: 'z', groupId: 7, groupName: 'amigos da facul' });
    const folders = groupFriends([solto, duda, ana], [group(2, 'duo', 0), group(9, 'vazia', 1)]);
    expect(folders.map(f => [f.name, f.friends.map(x => x.id)])).toEqual([
      ['duo', ['duda']], ['Geral', ['ana']], ['amigos da facul', ['z']],
    ]);
  });

  it.each([
    [{ type: 'league', status: 'inLobby', queueId: 450, championId: null, since: null }, 'No lobby · ARAM'],
    [{ type: 'league', status: 'inQueue', queueId: 999, championId: null, since: NOW - 90_000 }, 'Na fila · 1 min'],
    [{ type: 'league', status: 'outOfGame', queueId: 420, championId: null, since: null }, 'No client'],
    [{ type: 'league', status: 'inGame', queueId: null, championId: null, since: NOW + 60_000 }, 'Em partida'],
    [{ type: 'otherGame', productName: 'VALORANT' }, 'Jogando VALORANT'],
    [{ type: 'otherGame', productName: '' }, 'Em outro jogo da Riot'],
  ] as const)('descreve %o', (activity, text) => {
    expect(describeActivity(friend({ activity }), NOW)).toBe(text);
  });

  it('mostra a disponibilidade sem atividade', () => {
    expect(describeActivity(friend({ availability: 'mobile' }), NOW)).toBe('No celular');
  });
});

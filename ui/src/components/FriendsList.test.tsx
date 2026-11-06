import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import { describeActivity, friendDisplayName } from '../agent/friends';
import type { Friend } from '../bindings/Friend';
import { testFriend } from '../test/events';
import { makeState } from '../test/state';
import { FriendsList } from './FriendsList';

const NOW = Date.UTC(2026, 10, 6, 12, 0, 0);
const friend = (overrides: Partial<Friend>): Friend => ({ ...testFriend, ...overrides });
const connectedWith = (friends: Friend[]): ConnectionSnapshot => ({ status: 'connected', error: null, state: makeState({ friends }) });

const ana = friend({ id: 'ana', gameName: 'Ana', tagLine: 'BR1', profileIconId: 29,
  activity: { type: 'league', status: 'inGame', queueId: 420, championId: 266, since: NOW - 12 * 60_000 } });
const bia = friend({ id: 'bia', gameName: 'Bia', tagLine: 'BR1', availability: 'away', statusMessage: 'volto já' });
const caio = friend({ id: 'caio', gameName: '', tagLine: '', name: 'CaioAntigo', availability: 'offline' });

afterEach(() => { vi.useRealTimers(); });

describe('FriendsList', () => {
  it('pede para aguardar sem conexão com o LoL', () => {
    render(<FriendsList connection={{ status: 'reconnecting', error: null, state: null }} />);
    expect(screen.getByRole('region', { name: 'Amigos' })).toHaveTextContent('Aguarde a conexão');
  });

  it('avisa quando a lista está vazia', () => {
    render(<FriendsList connection={connectedWith([])} />);
    expect(screen.getByText('Você ainda não tem amigos na lista.')).toBeInTheDocument();
  });

  it('separa online e offline, mantendo a ordem do agente', () => {
    vi.useFakeTimers({ now: NOW });
    render(<FriendsList connection={connectedWith([ana, bia, caio])} />);
    expect(screen.getByRole('heading', { name: /Amigos/ })).toHaveTextContent('(2 de 3 online)');
    const online = within(screen.getByRole('list', { name: 'Amigos online' })).getAllByRole('listitem');
    expect(online.map(item => item.textContent)).toEqual([
      expect.stringContaining('Ana#BR1'), expect.stringContaining('Bia#BR1'),
    ]);
    expect(online[0]).toHaveTextContent('Em partida · Ranqueada Solo/Duo · 12 min');
    expect(within(online[0]).getByRole('img', { name: 'Online' })).toBeInTheDocument();
    expect(within(online[0]).getByRole('img', { name: 'Campeão da partida' })).toHaveAttribute('src', 'http://127.0.0.1:47800/assets/champion/266');
    expect(online[1]).toHaveTextContent('Ausente');
    expect(online[1]).toHaveTextContent('volto já');
    const offline = within(screen.getByRole('list', { name: 'Amigos offline', hidden: true })).getAllByRole('listitem', { hidden: true });
    expect(offline).toHaveLength(1);
    expect(offline[0]).toHaveTextContent('CaioAntigo');
    expect(screen.getByText('Offline (1)')).toBeInTheDocument();
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
    expect(screen.queryByRole('list', { name: 'Amigos online' })).not.toBeInTheDocument();
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

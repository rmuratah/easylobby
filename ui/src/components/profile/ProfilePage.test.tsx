import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { FriendProfilePage, ProfilePage } from './ProfilePage';
import { testFriend } from '../../test/events';
import type { PlayerProfile } from '../../bindings/PlayerProfile';
import { makeState } from '../../test/state';
import { match } from '../../test/profile';
import type { AgentState } from '../../bindings/AgentState';
import type { AgentClient, ConnectionSnapshot } from '../../agent/client';

const summoner = {
  gameName: 'Jogador', tagLine: 'BR1', displayName: '', summonerLevel: 87, profileIconId: 29,
  puuid: 'fake-puuid-0001', summonerId: 1,
};
const unranked = { tier: null, division: null, leaguePoints: 0, wins: 0, losses: 0, provisional: false };

function setup(overrides: Partial<AgentState> = {}) {
  const send = vi.fn(() => Promise.resolve({ status: 'ok' as const }));
  const connection: ConnectionSnapshot = { status: 'connected', error: null, state: makeState({ summoner, ...overrides }) };
  const view = render(<ProfilePage connection={connection} send={send} />);
  return { send, rerender: (next: Partial<AgentState>) => view.rerender(
    <ProfilePage connection={{ ...connection, state: makeState({ summoner, ...overrides, ...next }) }} send={send} />) };
}

describe('ProfilePage', () => {
  it('sem invocador, explica o que fazer', () => {
    render(<ProfilePage connection={{ status: 'connected', error: null, state: makeState() }} send={vi.fn()} />);
    expect(screen.getByText('Perfil indisponível')).toBeInTheDocument();
  });

  it('mostra cabeçalho, ranks, maestria e resumo na visão geral', () => {
    setup({
      ranked: { soloDuo: { ...unranked, tier: 'GOLD', division: 'II', leaguePoints: 47, wins: 30, losses: 10 }, flex: unranked },
      championMasteries: [{ championId: 103, level: 23, points: 287450 }],
      championCatalog: { champions: [{ id: 103, name: 'Ahri', alias: 'Ahri', roles: [], positions: [] }] },
      matchHistory: { matches: [match(1), match(2, { result: 'loss' })], hasMore: false },
    });
    expect(screen.getByRole('heading', { name: 'Jogador#BR1' })).toBeInTheDocument();
    expect(screen.getByText('Nível 87')).toBeInTheDocument();
    const solo = screen.getByRole('article', { name: 'Ranqueada Solo/Duo' });
    expect(solo).toHaveTextContent('Ouro II · 47 PDL');
    expect(solo.querySelector('img')?.getAttribute('src')).toMatch(/\/assets\/rankedEmblem\/4$/);
    expect(screen.getByRole('article', { name: 'Ranqueada Flex' }).querySelector('img')).toBeNull();
    expect(solo).toHaveTextContent('30V 10D · 75% de vitórias');
    expect(screen.getByRole('article', { name: 'Ranqueada Flex' })).toHaveTextContent('Sem classificação');
    expect(screen.getAllByText('Ahri').length).toBeGreaterThan(0);
    expect(screen.getByText('287.450 pontos', { exact: false })).toBeInTheDocument();
    expect(screen.getByText('Últimas 2 partidas')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Visão geral' })).toHaveAttribute('aria-selected', 'true');
  });

  it('copia nome#tag pelo ícone ao lado do nome', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    setup();
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Copiar Jogador#BR1' })); });
    expect(writeText).toHaveBeenCalledWith('Jogador#BR1');
    const status = screen.getByRole('status');
    expect(status).toHaveTextContent('Jogador #BR1');
    expect(status).toHaveTextContent('Copiado para a área de transferência');
  });

  it('se a cópia falhar, mostra o Riot ID para copiar à mão', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn(() => Promise.reject(new Error('negado'))) }, configurable: true,
    });
    setup();
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Copiar Jogador#BR1' })); });
    expect(screen.getByRole('status')).toHaveTextContent('Não deu para copiar');
  });

  it('filtra o histórico, carrega mais e abre o placar', () => {
    const { send, rerender } = setup({
      matchHistory: {
        matches: [match(1), match(2, { queueId: 450, gameMode: 'ARAM', result: 'loss' })],
        hasMore: true,
      },
    });
    fireEvent.click(screen.getByRole('tab', { name: 'Histórico' }));
    const list = screen.getByRole('list', { name: 'Partidas' });
    expect(within(list).getAllByRole('button')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'ARAM' }));
    expect(within(list).getAllByRole('button')).toHaveLength(1);
    expect(within(list).getByRole('button')).toHaveAccessibleName(/^Derrota/);
    fireEvent.click(screen.getByRole('button', { name: 'Flex' }));
    expect(screen.getByText('Nenhuma partida desta fila entre as 2 carregadas.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Carregar mais' }));
    expect(send).toHaveBeenCalledWith({ type: 'loadMoreMatches' });

    fireEvent.click(screen.getByRole('button', { name: 'Todas' }));
    const first = within(screen.getByRole('list', { name: 'Partidas' })).getAllByRole('button')[0];
    fireEvent.click(first);
    expect(send).toHaveBeenCalledWith({ type: 'openMatchDetails', gameId: 1 });
    expect(first).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('Carregando o placar…')).toBeInTheDocument();
    const players = Array.from({ length: 10 }, (_, i) => ({
      ...match(1).player, puuid: `p${i}`, gameName: `Jogador ${i}`, teamId: i < 5 ? 100 : 200, win: i < 5,
    }));
    rerender({
      matchHistory: { matches: [match(1)], hasMore: false },
      matchDetails: { ...match(1), remake: false, players },
    });
    expect(screen.getByText('Time azul · Vitória')).toBeInTheDocument();
    expect(screen.getByText('Time vermelho · Derrota')).toBeInTheDocument();
    expect(screen.getAllByRole('row')).toHaveLength(12);
    expect(screen.queryByRole('button', { name: 'Carregar mais' })).not.toBeInTheDocument();
  });
});

describe('jogadores do placar (#308)', () => {
  it('abrem o perfil, menos o dono do perfil e quem veio sem puuid', () => {
    const onOpenProfile = vi.fn();
    const players = Array.from({ length: 10 }, (_, i) => ({
      ...match(1).player, puuid: i === 9 ? '' : i === 0 ? summoner.puuid : `p${i}`, gameName: `Jogador ${i}`, tagLine: 'BR1',
      teamId: i < 5 ? 100 : 200, win: i < 5,
    }));
    const state = makeState({ summoner, matchHistory: { matches: [match(1)], hasMore: false }, matchDetails: { ...match(1), remake: false, players } });
    render(<ProfilePage connection={{ status: 'connected', error: null, state }} send={vi.fn(() => Promise.resolve({ status: 'ok' as const }))}
      onOpenProfile={onOpenProfile} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Histórico' }));
    fireEvent.click(within(screen.getByRole('list', { name: 'Partidas' })).getAllByRole('button')[0]);
    fireEvent.click(screen.getByRole('button', { name: 'Ver perfil de Jogador 3#BR1' }));
    expect(onOpenProfile).toHaveBeenCalledWith('p3');
    expect(screen.getAllByRole('button', { name: /^Ver perfil de/ })).toHaveLength(8);
    expect(screen.queryByRole('button', { name: 'Ver perfil de Jogador 0#BR1' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Ver perfil de Jogador 9#BR1' })).toBeNull();
    expect(screen.getByText('Jogador 9')).toBeInTheDocument();
  });

  it('sem onOpenProfile, os nomes não viram botão', () => {
    const players = [{ ...match(1).player, puuid: 'p1', gameName: 'Outro', teamId: 100 }];
    const state = makeState({ summoner, matchHistory: { matches: [match(1)], hasMore: false }, matchDetails: { ...match(1), remake: false, players } });
    render(<ProfilePage connection={{ status: 'connected', error: null, state }} send={vi.fn(() => Promise.resolve({ status: 'ok' as const }))} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Histórico' }));
    fireEvent.click(within(screen.getByRole('list', { name: 'Partidas' })).getAllByRole('button')[0]);
    expect(screen.getByText('Outro')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Ver perfil de/ })).toBeNull();
  });
});

describe('FriendProfilePage', () => {
  const amigo = { ...summoner, gameName: 'Amigo', puuid: 'amigo', summonerLevel: 143, summonerId: 1003 };
  const friend = { ...testFriend, activity: { type: 'league' as const, status: 'inGame' as const, queueId: 420, championId: 103, since: Date.now() } };
  const profile = (overrides: Partial<PlayerProfile> = {}): PlayerProfile => ({
    summoner: amigo, privateProfile: false,
    ranked: { soloDuo: { ...unranked, tier: 'PLATINUM', division: 'IV', leaguePoints: 12, wins: 51, losses: 47 }, flex: unranked },
    championMasteries: [{ championId: 103, level: 12, points: 184000 }],
    matchHistory: { matches: [match(1, {}, { puuid: 'amigo', gameName: 'Amigo' })], hasMore: true },
    ...overrides,
  });

  function setupFriend(playerProfile: PlayerProfile | null, send: AgentClient['send'] = vi.fn(() => Promise.resolve({ status: 'ok' as const }))) {
    const onBack = vi.fn();
    const onChat = vi.fn();
    const state = (p: PlayerProfile | null) => makeState({ summoner, friends: [friend], playerProfile: p });
    const view = render(<FriendProfilePage connection={{ status: 'connected', error: null, state: state(playerProfile) }} send={send}
      puuid="amigo" onBack={onBack} onChat={onChat} />);
    const rerender = (p: PlayerProfile | null) => view.rerender(<FriendProfilePage connection={{ status: 'connected', error: null, state: state(p) }}
      send={send} puuid="amigo" onBack={onBack} onChat={onChat} />);
    return { send, onBack, onChat, rerender };
  }

  it('pede o perfil ao abrir e mostra carregando até ele chegar', () => {
    const { send, rerender } = setupFriend(null);
    expect(send).toHaveBeenCalledWith({ type: 'openPlayerProfile', puuid: 'amigo' });
    expect(screen.getByText('Carregando o perfil…')).toBeInTheDocument();
    // O perfil de outro jogador que ainda está no estado não aparece.
    rerender(profile({ summoner: { ...amigo, puuid: 'outro', gameName: 'Outro' } }));
    expect(screen.getByText('Carregando o perfil…')).toBeInTheDocument();
    rerender(profile());
    expect(screen.getByRole('heading', { name: 'Amigo#BR1' })).toBeInTheDocument();
    expect(screen.getByText('Nível 143')).toBeInTheDocument();
    expect(screen.getByRole('article', { name: 'Ranqueada Solo/Duo' })).toHaveTextContent('Platina IV · 12 PDL');
    expect(screen.getByText('Últimas 1 partidas')).toBeInTheDocument();
  });

  it('mostra o status, conversa, volta e carrega mais partidas do amigo', () => {
    const { send, onBack, onChat } = setupFriend(profile());
    expect(screen.getByText(/^Em partida/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Conversar' }));
    expect(onChat).toHaveBeenCalledWith(friend.id);
    fireEvent.click(screen.getByRole('button', { name: 'Voltar' }));
    expect(onBack).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('tab', { name: 'Histórico' }));
    fireEvent.click(screen.getByRole('button', { name: 'Carregar mais' }));
    expect(send).toHaveBeenCalledWith({ type: 'loadMorePlayerMatches' });
  });

  it('avisa quando o perfil é privado ou uma parte não veio', () => {
    setupFriend(profile({ privateProfile: true, ranked: null, championMasteries: [], matchHistory: { matches: [], hasMore: false } }));
    expect(screen.getByText('Perfil privado')).toBeInTheDocument();
    expect(screen.getByRole('article', { name: 'Ranqueada Flex' })).toHaveTextContent('Indisponível');
    // No resumo da Visão geral e na aba Histórico.
    expect(screen.getAllByText('O histórico deste jogador é privado.')).toHaveLength(2);
    expect(screen.queryByRole('button', { name: 'Carregar mais' })).toBeNull();
  });

  it('mostra o erro e tenta de novo', async () => {
    const send = vi.fn(() => Promise.resolve({ status: 'error' as const, code: 'notFound' as const, message: 'jogador não encontrado' }));
    setupFriend(null, send);
    expect(await screen.findByText('Não foi possível carregar o perfil: jogador não encontrado')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    expect(send).toHaveBeenCalledTimes(2);
  });
});

import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ProfilePage } from './ProfilePage';
import { makeState } from '../../test/state';
import { match } from '../../test/profile';
import type { AgentState } from '../../bindings/AgentState';
import type { ConnectionSnapshot } from '../../agent/client';

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
      championCatalog: { champions: [{ id: 103, name: 'Ahri', alias: 'Ahri', roles: [] }] },
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

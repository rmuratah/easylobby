import { describe, expect, it } from 'vitest';
import { filterListings, listingFull, listingMapName } from './customGame';
import type { CustomGameListing } from '../bindings/CustomGameListing';

const game = (overrides: Partial<CustomGameListing>): CustomGameListing => ({
  partyId: 'p', name: 'Partida', owner: 'Dono', mapId: 11, map: 'summonersRift',
  players: 1, maxPlayers: 10, spectators: 0, maxSpectators: 4, spectatorPolicy: 'all', hasPassword: false, ...overrides,
});

describe('lista de personalizadas abertas (#264)', () => {
  it('mostra o mapa, mesmo quando o agente não conhece o valor', () => {
    expect(listingMapName(game({}))).toBe("Summoner's Rift");
    expect(listingMapName(game({ map: null, mapId: 30 }))).toBe('Mapa 30');
  });

  it('busca pelo nome ou pelo dono, sem maiúsculas nem acentos', () => {
    const games = [game({ partyId: '1', name: 'Treino do time' }), game({ partyId: '2', name: 'ARAM', owner: 'Ângela' })];
    expect(filterListings(games, '  TREINO ').map(g => g.partyId)).toEqual(['1']);
    expect(filterListings(games, 'angela').map(g => g.partyId)).toEqual(['2']);
    expect(filterListings(games, '').map(g => g.partyId)).toEqual(['1', '2']);
  });

  it('marca a partida cheia', () => {
    expect(listingFull(game({ players: 10 }))).toBe(true);
    expect(listingFull(game({ players: 9 }))).toBe(false);
    expect(listingFull(game({ players: 0, maxPlayers: 0 }))).toBe(false);
  });
});

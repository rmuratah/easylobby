import { describe, expect, it } from 'vitest';
import { filterListings, listingFull, listingMapName, listingModeName } from './customGame';
import type { CustomGameListing } from '../bindings/CustomGameListing';

const game = (overrides: Partial<CustomGameListing>): CustomGameListing => ({
  id: 1, name: 'Partida', owner: 'Dono', mapId: 11, map: 'summonersRift', gameType: 'DRAFT_MODE', pickType: 'draftMode',
  players: 1, maxPlayers: 10, spectators: 0, maxSpectators: 4, spectatorPolicy: 'all', hasPassword: false, ...overrides,
});

describe('lista de personalizadas abertas (#264)', () => {
  it('mostra mapa e modo, mesmo quando o agente não conhece o valor', () => {
    expect(listingMapName(game({}))).toBe("Summoner's Rift");
    expect(listingMapName(game({ map: null, mapId: 30 }))).toBe('Mapa 30');
    expect(listingModeName(game({ map: 'classicRift' }))).toBe('Escolha Alternada');
    expect(listingModeName(game({ pickType: null, gameType: 'PRACTICE_GAME' }))).toBe('PRACTICE_GAME');
    expect(listingModeName(game({ pickType: null, gameType: '' }))).toBe('—');
  });

  it('busca pelo nome ou pelo dono, sem maiúsculas nem acentos', () => {
    const games = [game({ id: 1, name: 'Treino do time' }), game({ id: 2, name: 'ARAM', owner: 'Ângela' })];
    expect(filterListings(games, '  TREINO ').map(g => g.id)).toEqual([1]);
    expect(filterListings(games, 'angela').map(g => g.id)).toEqual([2]);
    expect(filterListings(games, '').map(g => g.id)).toEqual([1, 2]);
  });

  it('marca a partida cheia', () => {
    expect(listingFull(game({ players: 10 }))).toBe(true);
    expect(listingFull(game({ players: 9 }))).toBe(false);
    expect(listingFull(game({ players: 0, maxPlayers: 0 }))).toBe(false);
  });
});

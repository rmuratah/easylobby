import { describe, expect, it } from 'vitest';
import { livePlayer, liveStats } from '../test/liveGame';
import { liveTags, tierLabel, tierShort } from './liveGame';

const labels = (player = livePlayer()) => liveTags(player).map(t => t.label);

describe('partida ao vivo', () => {
  it('nomeia e abrevia o elo', () => {
    expect(tierLabel({ tier: 'GOLD', division: 'II' })).toBe('Ouro II');
    expect(tierLabel({ tier: 'MASTER', division: null })).toBe('Mestre');
    expect(tierShort('DIAMOND', 'I')).toBe('D1');
    expect(tierShort('PLATINUM', 'IV')).toBe('PL4');
    expect(tierShort('GRANDMASTER', null)).toBe('GM');
    expect(tierShort('CHALLENGER', null)).toBe('DS');
  });

  it('marca sequências de 3 ou mais', () => {
    expect(labels(livePlayer({ stats: liveStats({ streak: 3 }) }))).toContain('3 vitórias seguidas');
    expect(labels(livePlayer({ stats: liveStats({ streak: -5 }) }))).toContain('5 derrotas seguidas');
    expect(labels(livePlayer({ stats: liveStats({ streak: 2 }) }))).toEqual([]);
  });

  it('marca campeão novo e especialista', () => {
    expect(labels(livePlayer({ stats: liveStats({ championGames: 0 }) }))).toEqual(['Novo no campeão']);
    // Com poucas partidas no histórico, não dá para dizer.
    expect(labels(livePlayer({ stats: liveStats({ championGames: 0, recentGames: 6 }) }))).toEqual([]);
    expect(labels(livePlayer({ stats: liveStats({ championGames: 8, championWins: 5 }) }))).toEqual(['Especialista']);
    expect(labels(livePlayer({ stats: liveStats({ championGames: 5, recentGames: 20 }) }))).toEqual([]);
  });

  it('marca fora da rota só com cargo e rota conhecidos', () => {
    expect(labels(livePlayer({ position: 'top' }))).toEqual(['Fora da rota']);
    expect(labels(livePlayer({ position: 'unselected' }))).toEqual([]);
    expect(labels(livePlayer({ position: 'top', stats: liveStats({ mainPosition: 'unselected' }) }))).toEqual([]);
    expect(liveTags(livePlayer({ stats: null }))).toEqual([]);
  });
});

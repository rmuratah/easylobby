import { describe, expect, it } from 'vitest';
import { match } from '../test/profile';
import {
  formatDuration, kda, matchesFilter, matchStats, queueLabel, rankLabel, tierEmblemId, timeAgo, winRate,
} from './profile';

const entry = { tier: null, division: null, leaguePoints: 0, wins: 0, losses: 0, provisional: false };

describe('perfil', () => {
  it('nomeia o rank em português', () => {
    expect(rankLabel({ ...entry, tier: 'GOLD', division: 'II' })).toBe('Ouro II');
    expect(rankLabel({ ...entry, tier: 'GRANDMASTER' })).toBe('Grão-Mestre');
    expect(rankLabel(entry)).toBe('Sem classificação');
    expect(winRate(3, 1)).toBe(75);
    expect(winRate(0, 0)).toBeNull();
  });

  it('acha o emblema de cada tier', () => {
    expect(tierEmblemId('IRON')).toBe(1);
    expect(tierEmblemId('EMERALD')).toBe(6);
    expect(tierEmblemId('DIAMOND')).toBe(7);
    expect(tierEmblemId('CHALLENGER')).toBe(10);
    expect(tierEmblemId('UNRANKED')).toBeNull();
  });

  it('formata tempo, duração e KDA', () => {
    expect(formatDuration(1805)).toBe('30:05');
    expect(formatDuration(3725)).toBe('1:02:05');
    expect(timeAgo(0, 30_000)).toBe('agora');
    expect(timeAgo(0, 5 * 60_000)).toBe('há 5 min');
    expect(timeAgo(0, 3 * 3_600_000)).toBe('há 3 h');
    expect(timeAgo(0, 50 * 3_600_000)).toBe('há 2 dias');
    expect(kda(6, 2, 8)).toBe('7,0');
    expect(kda(6, 0, 8)).toBe('Perfeito');
  });

  it('nomeia filas e filtra', () => {
    expect(queueLabel(420, [])).toBe('Ranqueada Solo/Duo');
    expect(queueLabel(0, [])).toBe('Personalizada');
    expect(queueLabel(9999, [])).toBe('Fila 9999');
    const aram = match(1, { queueId: 2400, gameMode: 'KIWI' });
    expect(matchesFilter(aram, 'aram')).toBe(true);
    expect(matchesFilter(aram, 'soloDuo')).toBe(false);
    expect(matchesFilter(match(2, { queueId: 440 }), 'flex')).toBe(true);
    expect(matchesFilter(match(3, { queueId: 400 }), 'normal')).toBe(true);
  });

  it('resume as partidas sem contar remakes', () => {
    const stats = matchStats([
      match(1),
      match(2, { result: 'loss' }, { championId: 64, kills: 2, deaths: 6, assists: 4 }),
      match(3, {}, { championId: 64 }),
      match(4, { result: 'remake' }, { championId: 222 }),
    ]);
    expect(stats.games).toBe(3);
    expect([stats.wins, stats.losses, stats.winRate]).toEqual([2, 1, 67]);
    expect(stats.champions.map(c => c.championId)).toEqual([64, 103]);
    expect(stats.deaths).toBeCloseTo(10 / 3);
  });
});

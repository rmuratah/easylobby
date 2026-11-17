import { describe, expect, it } from 'vitest';
import type { LpPoint } from '../bindings/LpPoint';
import { daysAgo, lpScore, lpSeries, pointLabel, scoreLabel } from './lpChart';

const DAY = 24 * 60 * 60 * 1000;
const NOW = Date.UTC(2026, 10, 17, 12);
const point = (daysBefore: number, tier: string, division: string | null, leaguePoints: number): LpPoint =>
  ({ at: NOW - daysBefore * DAY, tier, division, leaguePoints, wins: 10, losses: 5 });

describe('escala do gráfico de PDL', () => {
  it('soma divisões de 100 PDL e continua no Mestre', () => {
    expect(lpScore({ tier: 'IRON', division: 'IV', leaguePoints: 0 })).toBe(0);
    expect(lpScore({ tier: 'GOLD', division: 'II', leaguePoints: 47 })).toBe(1200 + 200 + 47);
    expect(lpScore({ tier: 'DIAMOND', division: 'I', leaguePoints: 100 })).toBe(2800);
    expect(lpScore({ tier: 'MASTER', division: null, leaguePoints: 0 })).toBe(2800);
    expect(lpScore({ tier: 'CHALLENGER', division: null, leaguePoints: 1089 })).toBe(3889);
  });

  it('dá nome às linhas de grade', () => {
    expect(scoreLabel(1400)).toBe('Ouro II');
    expect(scoreLabel(0)).toBe('Ferro IV');
    expect(scoreLabel(2800)).toBe('Mestre');
    expect(scoreLabel(3000)).toBe('200 PDL');
    expect(pointLabel(point(0, 'MASTER', null, 120))).toBe('Mestre · 120 PDL');
  });
});

describe('lpSeries', () => {
  it('sem pontos, não há gráfico', () => {
    expect(lpSeries([], NOW)).toBeNull();
  });

  it('calcula o ganho do período e o pico, a partir do último ponto antes da janela', () => {
    const series = lpSeries([
      point(40, 'GOLD', 'IV', 50), // antes da janela: vira o começo
      point(20, 'GOLD', 'III', 10),
      point(10, 'GOLD', 'II', 80),
      point(2, 'GOLD', 'II', 60),
    ], NOW)!;
    expect(series.points).toHaveLength(4);
    expect(series.points[0].at).toBe(series.start);
    expect(series.gain).toBe(1460 - 1250);
    expect(pointLabel(series.peak)).toBe('Ouro II · 80 PDL');
    expect(series.grid.every(v => v >= series.min && v <= series.max)).toBe(true);
    expect(series.grid.length).toBeLessThanOrEqual(4);
  });

  it('com um ponto só, a altura mínima é de uma divisão', () => {
    const series = lpSeries([point(3, 'SILVER', 'I', 20)], NOW)!;
    expect(series.gain).toBe(0);
    expect(series.max - series.min).toBeGreaterThanOrEqual(100);
  });
});

describe('daysAgo', () => {
  it('fala em dias', () => {
    expect(daysAgo(NOW - 2 * 60 * 60 * 1000, NOW)).toBe('hoje');
    expect(daysAgo(NOW - DAY - 1, NOW)).toBe('ontem');
    expect(daysAgo(NOW - 11 * DAY, NOW)).toBe('há 11 dias');
  });
});

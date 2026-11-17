import type { LpPoint } from '../bindings/LpPoint';
import { tierName } from './profile';

/** Tiers em ordem, do mais baixo para o mais alto. */
const tiers = ['IRON', 'BRONZE', 'SILVER', 'GOLD', 'PLATINUM', 'EMERALD', 'DIAMOND', 'MASTER', 'GRANDMASTER', 'CHALLENGER'];
const divisions = ['IV', 'III', 'II', 'I'];
/** Pontos do começo do Mestre na escala contínua: 7 tiers de 4 divisões de 100 PDL. */
const MASTER_BASE = 7 * 400;
const DAY_MS = 24 * 60 * 60 * 1000;

/** Janela do gráfico, em dias. */
export const LP_WINDOW_DAYS = 30;

/**
 * Rank numa escala contínua, para o eixo do gráfico: cada divisão vale 100
 * pontos (Ferro IV 0 PDL = 0, Ouro IV 0 PDL = 1200). Do Mestre para cima, o
 * PDL soma direto a partir de 2800, porque Grão-Mestre e Desafiante só
 * mudam pelo PDL.
 */
export function lpScore(point: Pick<LpPoint, 'tier' | 'division' | 'leaguePoints'>): number {
  const tier = Math.max(0, tiers.indexOf(point.tier));
  if (tier >= 7) return MASTER_BASE + point.leaguePoints;
  const division = Math.max(0, divisions.indexOf(point.division ?? 'IV'));
  return tier * 400 + division * 100 + Math.min(point.leaguePoints, 100);
}

/** Nome da linha de grade no valor `score` da escala: `Ouro II`, ou `Mestre 200` acima do Mestre. */
export function scoreLabel(score: number): string {
  if (score >= MASTER_BASE) {
    const lp = score - MASTER_BASE;
    return lp === 0 ? 'Mestre' : `${lp} PDL`;
  }
  const tier = Math.floor(score / 400);
  const division = Math.floor((score % 400) / 100);
  return `${tierName(tiers[tier])} ${divisions[division]}`;
}

/** `Ouro II · 47 PDL` ou `Mestre · 120 PDL`. */
export function pointLabel(point: LpPoint): string {
  const rank = point.division ? `${tierName(point.tier)} ${point.division}` : tierName(point.tier);
  return `${rank} · ${point.leaguePoints} PDL`;
}

export type LpSeries = {
  /** Pontos dentro da janela, do mais velho para o mais novo. O primeiro pode ser o último ponto de antes da janela, levado para o começo dela. */
  points: { point: LpPoint; at: number; score: number }[];
  start: number;
  end: number;
  /** Diferença de pontos na escala entre o começo e o fim da janela. */
  gain: number;
  /** Ponto mais alto da janela. */
  peak: LpPoint;
  /** Grade horizontal: valores da escala, de baixo para cima. */
  grid: number[];
  min: number;
  max: number;
};

/**
 * Monta o gráfico dos últimos `days` dias a partir de `history` (ordenado do
 * mais velho para o mais novo). `null` sem nenhum ponto até `now`.
 */
export function lpSeries(history: LpPoint[], now: number, days = LP_WINDOW_DAYS): LpSeries | null {
  const start = now - days * DAY_MS;
  const inside = history.filter(p => p.at >= start && p.at <= now);
  const before = history.filter(p => p.at < start).at(-1);
  const points = [
    ...(before ? [{ point: before, at: start, score: lpScore(before) }] : []),
    ...inside.map(p => ({ point: p, at: p.at, score: lpScore(p) })),
  ];
  if (points.length === 0) return null;
  const scores = points.map(p => p.score);
  let min = Math.min(...scores);
  let max = Math.max(...scores);
  const peak = points.reduce((best, p) => (p.score > best.score ? p : best)).point;
  // Pelo menos uma divisão de altura, para a linha não colar nas bordas.
  if (max - min < 100) {
    const middle = (max + min) / 2;
    min = middle - 50;
    max = middle + 50;
  }
  const pad = (max - min) * 0.12;
  min -= pad;
  max += pad;
  return {
    points, start, end: now, gain: scores.at(-1)! - scores[0], peak, grid: gridLines(min, max), min, max,
  };
}

/** Linhas de grade nas divisões (ou nos tiers, se forem muitas), e de 100 em 100 PDL acima do Mestre. */
function gridLines(min: number, max: number): number[] {
  for (const step of [100, 200, 400, 800]) {
    const lines: number[] = [];
    for (let v = Math.ceil(min / step) * step; v <= max; v += step) lines.push(v);
    if (lines.length <= 4) return lines;
  }
  return [];
}

/** `hoje`, `ontem` ou `há 11 dias`. */
export function daysAgo(at: number, now: number): string {
  const days = Math.floor((now - at) / DAY_MS);
  if (days <= 0) return 'hoje';
  if (days === 1) return 'ontem';
  return `há ${days} dias`;
}

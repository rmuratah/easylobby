import type { LivePlayer } from '../bindings/LivePlayer';
import type { TierDivision } from '../bindings/TierDivision';
import { tierName } from './profile';

/** Fases em que a tela de partida ao vivo aparece (#268). */
export const liveGamePhases = new Set(['GameStart', 'InProgress', 'Reconnect']);

/** `Ouro II`, `Mestre`. */
export function tierLabel(tier: TierDivision): string {
  return tier.division ? `${tierName(tier.tier)} ${tier.division}` : tierName(tier.tier);
}

/** Sigla do elo para o selo compacto: `O2`, `M`, `GM`, `D1` (Diamante), `DS` (Desafiante). */
export function tierShort(tier: string, division: string | null): string {
  const letters: Record<string, string> = {
    IRON: 'F', BRONZE: 'B', SILVER: 'P', GOLD: 'O', PLATINUM: 'PL', EMERALD: 'E', DIAMOND: 'D',
    MASTER: 'M', GRANDMASTER: 'GM', CHALLENGER: 'DS',
  };
  const romans: Record<string, number> = { I: 1, II: 2, III: 3, IV: 4 };
  const letter = letters[tier] ?? tier.charAt(0);
  return division && romans[division] ? `${letter}${romans[division]}` : letter;
}

export type LiveTag = { label: string; tone: 'good' | 'bad' | 'neutral'; title: string };

/**
 * Etiquetas de um jogador, das partidas recentes que o agente leu (#268).
 * Critérios:
 * - 3 ou mais vitórias (ou derrotas) seguidas nas partidas recentes;
 * - "Novo no campeão": nenhuma partida recente com o campeão, entre pelo menos 10;
 * - "Especialista": 5 ou mais partidas com o campeão, e ele é pelo menos 40% das recentes;
 * - "Fora da rota": o cargo desta partida não é a rota mais jogada (só nas filas com cargos).
 */
export function liveTags(player: LivePlayer): LiveTag[] {
  const stats = player.stats;
  if (!stats) return [];
  const tags: LiveTag[] = [];
  if (stats.streak >= 3) {
    tags.push({ label: `${stats.streak} vitórias seguidas`, tone: 'good', title: 'Venceu as últimas partidas em sequência.' });
  } else if (stats.streak <= -3) {
    tags.push({ label: `${-stats.streak} derrotas seguidas`, tone: 'bad', title: 'Perdeu as últimas partidas em sequência.' });
  }
  if (stats.recentGames >= 10 && stats.championGames === 0) {
    tags.push({ label: 'Novo no campeão', tone: 'neutral', title: `Nenhuma das últimas ${stats.recentGames} partidas com este campeão.` });
  } else if (stats.championGames >= 5 && stats.championGames * 5 >= stats.recentGames * 2) {
    tags.push({ label: 'Especialista', tone: 'good', title: `${stats.championGames} das últimas ${stats.recentGames} partidas com este campeão.` });
  }
  const lanes = ['top', 'jungle', 'middle', 'bottom', 'utility'];
  if (lanes.includes(player.position) && lanes.includes(stats.mainPosition) && player.position !== stats.mainPosition) {
    tags.push({ label: 'Fora da rota', tone: 'neutral', title: 'O cargo desta partida não é a rota que ele mais joga.' });
  }
  return tags;
}

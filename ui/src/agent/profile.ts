import type { RankedEntry } from '../bindings/RankedEntry';
import type { MatchSummary } from '../bindings/MatchSummary';
import type { GameQueue } from '../bindings/GameQueue';

const tierNames: Record<string, string> = {
  IRON: 'Ferro',
  BRONZE: 'Bronze',
  SILVER: 'Prata',
  GOLD: 'Ouro',
  PLATINUM: 'Platina',
  EMERALD: 'Esmeralda',
  DIAMOND: 'Diamante',
  MASTER: 'Mestre',
  GRANDMASTER: 'Grão-Mestre',
  CHALLENGER: 'Desafiante',
};

/** Tier em português (`GOLD` -> `Ouro`); tier desconhecido aparece como veio. */
export function tierName(tier: string): string {
  return tierNames[tier] ?? tier.charAt(0) + tier.slice(1).toLowerCase();
}

/** Ordem dos tiers no `id` do emblema (`/assets/rankedEmblem/<id>`), a mesma de `RANKED_TIERS` no agente. */
const emblemTiers = ['IRON', 'BRONZE', 'SILVER', 'GOLD', 'PLATINUM', 'EMERALD', 'DIAMOND', 'MASTER', 'GRANDMASTER', 'CHALLENGER'];

/** `id` do emblema do tier (`IRON` -> 1, `CHALLENGER` -> 10), ou `null` para um tier desconhecido. */
export function tierEmblemId(tier: string): number | null {
  const index = emblemTiers.indexOf(tier.toUpperCase());
  return index < 0 ? null : index + 1;
}

/** `Ouro II`, `Mestre` ou `Sem classificação`. */
export function rankLabel(entry: RankedEntry): string {
  if (!entry.tier) return 'Sem classificação';
  return entry.division ? `${tierName(entry.tier)} ${entry.division}` : tierName(entry.tier);
}

/** Taxa de vitória em porcentagem inteira, ou `null` sem partidas. */
export function winRate(wins: number, losses: number): number | null {
  const total = wins + losses;
  return total === 0 ? null : Math.round((wins / total) * 100);
}

/** Nomes das filas que não estão na lista de filas abertas (`queues`). */
const knownQueues: Record<number, string> = {
  400: 'Normal — Escolha alternada',
  420: 'Ranqueada Solo/Duo',
  430: 'Normal — Às cegas',
  440: 'Ranqueada Flex',
  450: 'ARAM',
  480: 'Jogo Dinâmico',
  490: 'Partida Rápida',
  700: 'Clash',
  720: 'Clash ARAM',
  830: 'Contra IA',
  840: 'Contra IA',
  850: 'Contra IA',
  870: 'Contra IA',
  880: 'Contra IA',
  890: 'Contra IA',
  900: 'URF',
  1700: 'Arena',
  1710: 'Arena',
  1740: 'Arena',
  1900: 'URF',
  2400: 'ARAM: Desordem',
  3140: 'Ferramenta de Treino',
};

/** Nome da fila de uma partida: a lista do agente primeiro, depois os conhecidos. */
export function queueLabel(queueId: number, queues: GameQueue[]): string {
  if (queueId === 0) return 'Personalizada';
  const known = knownQueues[queueId];
  if (known) return known;
  return queues.find(q => q.queueId === queueId)?.name || `Fila ${queueId}`;
}

export type MatchFilter = 'all' | 'soloDuo' | 'flex' | 'normal' | 'aram';

export const matchFilters: { value: MatchFilter; label: string }[] = [
  { value: 'all', label: 'Todas' },
  { value: 'soloDuo', label: 'Solo/Duo' },
  { value: 'flex', label: 'Flex' },
  { value: 'normal', label: 'Normal' },
  { value: 'aram', label: 'ARAM' },
];

const normalQueues = new Set([400, 430, 480, 490]);
const aramModes = new Set(['ARAM', 'KIWI']);

export function matchesFilter(match: MatchSummary, filter: MatchFilter): boolean {
  switch (filter) {
    case 'all': return true;
    case 'soloDuo': return match.queueId === 420;
    case 'flex': return match.queueId === 440;
    case 'normal': return normalQueues.has(match.queueId);
    case 'aram': return aramModes.has(match.gameMode);
  }
}

/** `32:05` ou `1:02:40`. */
export function formatDuration(totalSecs: number): string {
  const hours = Math.floor(totalSecs / 3600);
  const minutes = Math.floor((totalSecs % 3600) / 60);
  const seconds = String(totalSecs % 60).padStart(2, '0');
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${seconds}` : `${minutes}:${seconds}`;
}

/** `agora`, `há 5 min`, `há 3 h`, `há 2 dias`. */
export function timeAgo(createdAt: number, now: number): string {
  const minutes = Math.max(0, Math.floor((now - createdAt) / 60000));
  if (minutes < 1) return 'agora';
  if (minutes < 60) return `há ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `há ${hours} h`;
  const days = Math.floor(hours / 24);
  return days === 1 ? 'há 1 dia' : `há ${days} dias`;
}

/** KDA (abates + assistências por morte), com uma casa. Sem mortes, `Perfeito`. */
export function kda(kills: number, deaths: number, assists: number): string {
  if (deaths === 0) return 'Perfeito';
  return ((kills + assists) / deaths).toFixed(1).replace('.', ',');
}

export type MatchStats = {
  games: number;
  wins: number;
  losses: number;
  winRate: number | null;
  kills: number;
  deaths: number;
  assists: number;
  /** Os campeões mais jogados, do mais jogado para o menos, até 3. */
  champions: { championId: number; games: number; wins: number }[];
};

/** Resumo das partidas carregadas, sem contar os remakes. */
export function matchStats(matches: MatchSummary[]): MatchStats {
  const counted = matches.filter(m => m.result !== 'remake');
  const wins = counted.filter(m => m.result === 'win').length;
  const sum = (key: 'kills' | 'deaths' | 'assists') => counted.reduce((total, m) => total + m.player[key], 0);
  const byChampion = new Map<number, { championId: number; games: number; wins: number }>();
  for (const m of counted) {
    const entry = byChampion.get(m.player.championId) ?? { championId: m.player.championId, games: 0, wins: 0 };
    entry.games += 1;
    if (m.result === 'win') entry.wins += 1;
    byChampion.set(m.player.championId, entry);
  }
  const champions = [...byChampion.values()]
    .sort((a, b) => b.games - a.games || b.wins - a.wins || a.championId - b.championId)
    .slice(0, 3);
  const average = (n: number) => counted.length === 0 ? 0 : n / counted.length;
  return {
    games: counted.length,
    wins,
    losses: counted.length - wins,
    winRate: winRate(wins, counted.length - wins),
    kills: average(sum('kills')),
    deaths: average(sum('deaths')),
    assists: average(sum('assists')),
    champions,
  };
}

/** Fila cujo elo o card do lobby mostra (#275): a Flex no lobby da Flex, a Solo/Duo nos outros, como o client oficial. */
export function lobbyRankQueue(queueId: number): 'soloDuo' | 'flex' {
  return queueId === 440 ? 'flex' : 'soloDuo';
}

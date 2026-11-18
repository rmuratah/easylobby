import type { AdvancedStats } from '../bindings/AdvancedStats';
import type { GameResult } from '../bindings/GameResult';
import type { MatchPlayer } from '../bindings/MatchPlayer';
import type { Perk } from '../bindings/Perk';
import { kda } from './profile';

/** Um jogador na tabela de detalhes avançados (#586). */
export type AdvancedColumn = { player: MatchPlayer; stats: AdvancedStats; teamId: number; firstOfTeam: boolean };

/** Uma linha: o rótulo, o valor que decide o destaque e o texto da célula. */
export type AdvancedRow = {
  label: string;
  value: (column: AdvancedColumn) => number;
  text?: (column: AdvancedColumn) => string;
  /** O menor valor é o melhor (tempo morto). */
  lowerIsBetter?: boolean;
};

export type AdvancedCategory = { title: string; rows: AdvancedRow[]; collapsed?: boolean };

type NumericKey = Exclude<keyof AdvancedStats, 'perks'>;
const stat = (label: string, key: NumericKey): AdvancedRow => ({ label, value: c => c.stats[key] });
const time = (secs: number) => `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, '0')}`;

/** Chaves numéricas de `AdvancedStats`, para o validador do `/ws` (o `satisfies` pega campo novo). */
export const advancedStatsKeys = Object.keys({
  largestKillingSpree: true, largestMultiKill: true, doubleKills: true, tripleKills: true, quadraKills: true,
  pentaKills: true, largestCriticalStrike: true, physicalDamageToChampions: true, magicDamageToChampions: true,
  trueDamageToChampions: true, damageDealt: true, physicalDamageDealt: true, magicDamageDealt: true,
  trueDamageDealt: true, damageTaken: true, physicalDamageTaken: true, magicDamageTaken: true, trueDamageTaken: true,
  damageSelfMitigated: true, heal: true, healOnTeammates: true, shieldOnTeammates: true, timeCcingOthers: true,
  wardsPlaced: true, wardsKilled: true, controlWardsBought: true, goldSpent: true, minionsKilled: true,
  neutralMinionsKilled: true, allyJungleMinionsKilled: true, enemyJungleMinionsKilled: true, turretKills: true,
  turretTakedowns: true, inhibitorKills: true, damageToTurrets: true, damageToObjectives: true,
  damageToEpicMonsters: true, dragonKills: true, baronKills: true, objectivesStolen: true,
  longestTimeLivingSecs: true, timeDeadSecs: true, spell1Casts: true, spell2Casts: true, spell3Casts: true,
  spell4Casts: true, summoner1Casts: true, summoner2Casts: true, allInPings: true, assistMePings: true,
  basicPings: true, commandPings: true, dangerPings: true, enemyMissingPings: true, enemyVisionPings: true,
  getBackPings: true, holdPings: true, needVisionPings: true, onMyWayPings: true, pushPings: true,
  retreatPings: true, visionClearedPings: true,
} satisfies Record<NumericKey, true>);

/**
 * As categorias da tabela, na ordem e com os nomes do "Exibir detalhes
 * avançados" do client oficial; depois o que o client não mostra (tempo,
 * habilidades e pings), com habilidades e pings fechadas.
 */
export const advancedCategories: AdvancedCategory[] = [
  { title: 'Combate', rows: [
    { label: 'K / D / A', value: c => (c.player.kills + c.player.assists) / Math.max(1, c.player.deaths),
      text: c => `${c.player.kills} / ${c.player.deaths} / ${c.player.assists}` },
    { label: 'Razão', value: c => (c.player.kills + c.player.assists) / Math.max(1, c.player.deaths),
      text: c => kda(c.player.kills, c.player.deaths, c.player.assists) },
    stat('Maior sequência de abates', 'largestKillingSpree'),
    stat('Maior multiabate', 'largestMultiKill'),
    stat('Placar de controle de grupo', 'timeCcingOthers'),
    stat('Abates duplos', 'doubleKills'),
    stat('Abates triplos', 'tripleKills'),
    stat('Abates quádruplos', 'quadraKills'),
    stat('Pentakills', 'pentaKills'),
  ] },
  { title: 'Dano causado', rows: [
    { label: 'Dano total a campeões', value: c => c.player.damageToChampions },
    stat('Dano físico a campeões', 'physicalDamageToChampions'),
    stat('Dano mágico a campeões', 'magicDamageToChampions'),
    stat('Dano verdadeiro a campeões', 'trueDamageToChampions'),
    stat('Dano total causado', 'damageDealt'),
    stat('Dano físico causado', 'physicalDamageDealt'),
    stat('Dano mágico causado', 'magicDamageDealt'),
    stat('Dano verdadeiro causado', 'trueDamageDealt'),
    stat('Maior acerto crítico', 'largestCriticalStrike'),
    stat('Dano total a torres', 'damageToTurrets'),
    stat('Dano total aos objetivos', 'damageToObjectives'),
    stat('Dano a monstros épicos', 'damageToEpicMonsters'),
  ] },
  { title: 'Dano recebido e curado', rows: [
    stat('Dano curado', 'heal'),
    stat('Cura em aliados', 'healOnTeammates'),
    stat('Escudo em aliados', 'shieldOnTeammates'),
    stat('Dano recebido', 'damageTaken'),
    stat('Dano físico recebido', 'physicalDamageTaken'),
    stat('Dano mágico recebido', 'magicDamageTaken'),
    stat('Dano verdadeiro recebido', 'trueDamageTaken'),
    stat('Dano automitigado', 'damageSelfMitigated'),
  ] },
  { title: 'Visão', rows: [
    { label: 'Placar de visão', value: c => c.player.visionScore },
    stat('Sentinelas posicionadas', 'wardsPlaced'),
    stat('Sentinelas destruídas', 'wardsKilled'),
    stat('Sentinelas de controle compradas', 'controlWardsBought'),
  ] },
  { title: 'Receita', rows: [
    { label: 'Ouro recebido', value: c => c.player.gold },
    stat('Ouro gasto', 'goldSpent'),
    stat('Tropas abatidas', 'minionsKilled'),
    stat('Monstros neutros abatidos', 'neutralMinionsKilled'),
    stat('Monstros na selva aliada', 'allyJungleMinionsKilled'),
    stat('Monstros na selva inimiga', 'enemyJungleMinionsKilled'),
  ] },
  { title: 'Outros', rows: [
    stat('Torres destruídas', 'turretKills'),
    stat('Inibidores destruídos', 'inhibitorKills'),
    stat('Participação em torres', 'turretTakedowns'),
    stat('Dragões', 'dragonKills'),
    stat('Barões', 'baronKills'),
    stat('Objetivos roubados', 'objectivesStolen'),
    { label: 'Maior tempo vivo', value: c => c.stats.longestTimeLivingSecs, text: c => time(c.stats.longestTimeLivingSecs) },
    { label: 'Tempo morto', value: c => c.stats.timeDeadSecs, text: c => time(c.stats.timeDeadSecs), lowerIsBetter: true },
  ] },
  { title: 'Habilidades usadas', collapsed: true, rows: [
    stat('Q', 'spell1Casts'),
    stat('W', 'spell2Casts'),
    stat('E', 'spell3Casts'),
    stat('R', 'spell4Casts'),
    stat('Feitiço 1', 'summoner1Casts'),
    stat('Feitiço 2', 'summoner2Casts'),
  ] },
  { title: 'Pings', collapsed: true, rows: [
    stat('A caminho', 'onMyWayPings'),
    stat('Inimigo sumiu', 'enemyMissingPings'),
    stat('Ajuda', 'assistMePings'),
    stat('Recuar', 'getBackPings'),
    stat('Comando', 'commandPings'),
    stat('Atacar com tudo', 'allInPings'),
    stat('Avançar', 'pushPings'),
    stat('Precisa de visão', 'needVisionPings'),
    stat('Visão inimiga', 'enemyVisionPings'),
    stat('Visão limpa', 'visionClearedPings'),
    stat('Segurar', 'holdPings'),
    stat('Cuidado', 'dangerPings'),
    stat('Retirada', 'retreatPings'),
    stat('Simples', 'basicPings'),
  ] },
];

/** Os jogadores, time por time (o do jogador logado primeiro), com as estatísticas. */
export function advancedColumns(result: GameResult): AdvancedColumn[] {
  return result.teams.flatMap(team => team.players.flatMap((player, i) => {
    const stats = team.advanced[i];
    return stats ? [{ player, stats, teamId: team.teamId, firstOfTeam: i === 0 }] : [];
  }));
}

/**
 * Índices das colunas com o melhor valor da linha. Sem destaque quando todos
 * empatam (todos zero, por exemplo).
 */
export function bestColumns(row: AdvancedRow, columns: AdvancedColumn[]): Set<number> {
  const values = columns.map(row.value);
  const best = row.lowerIsBetter ? Math.min(...values) : Math.max(...values);
  if (values.every(v => v === best)) return new Set();
  return new Set(values.flatMap((v, i) => (v === best ? [i] : [])));
}

/** O texto da célula: o do `text` ou o número com separador de milhar. */
export function cellText(row: AdvancedRow, column: AdvancedColumn): string {
  return row.text ? row.text(column) : row.value(column).toLocaleString('pt-BR');
}

/** A partida teve runas: falso nos modos sem runas (ARAM: Desordem, Arena). */
export function hasRunes(result: GameResult): boolean {
  return result.teams.some(team => team.advanced.some(stats => stats.perks.length > 0));
}

/** Uma runa de um jogador, com as estatísticas dela já em texto. */
export type RuneResult = { id: number; name: string; lines: string[] };

/**
 * As runas do jogador com as estatísticas do fim da partida: as descrições do
 * catálogo (`Total de dano: @eogvar1@`) com os valores no lugar. Sem o
 * catálogo, só o id da runa. Os tempos vêm como a Riot monta, em partes
 * (`@eogvar1@:@eogvar2@@eogvar3@`), então os valores entram sem formatar.
 */
export function runeResults(stats: AdvancedStats, perks: Perk[]): RuneResult[] {
  return stats.perks.map(({ id, vars }) => {
    const perk = perks.find(p => p.id === id);
    const lines = (perk?.endOfGameStatDescs ?? []).map(desc => {
      const time = /@eogvar\d@:@eogvar\d@/.test(desc);
      return desc.replace(/@eogvar([123])@/g, (_, n: string) => {
        const value = vars[Number(n) - 1] ?? 0;
        return time ? String(value) : value.toLocaleString('pt-BR');
      });
    });
    return { id, name: perk?.name ?? '', lines };
  });
}

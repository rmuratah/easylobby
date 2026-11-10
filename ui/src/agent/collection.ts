import type { CollectionChampion } from '../bindings/CollectionChampion';
import type { CollectionSkin } from '../bindings/CollectionSkin';
import type { Lane } from './positions';
import { searchKey } from './champSelect';

/** Funções dos campeões como a LCU manda, na ordem do filtro da Coleção (#369). */
export const CHAMPION_ROLES = ['assassin', 'fighter', 'mage', 'marksman', 'support', 'tank'] as const;
export type ChampionRole = (typeof CHAMPION_ROLES)[number];

export const roleNames = {
  assassin: 'Assassino',
  fighter: 'Lutador',
  mage: 'Mago',
  marksman: 'Atirador',
  support: 'Suporte',
  tank: 'Tanque',
} satisfies Record<ChampionRole, string>;

/** O que a grade mostra: todos, só os da conta ou só os que faltam. */
export type OwnershipFilter = 'all' | 'owned' | 'missing';

export type CollectionFilters = {
  query: string;
  ownership: OwnershipFilter;
  role: ChampionRole | null;
  lane: Lane | null;
};

export const emptyFilters: CollectionFilters = { query: '', ownership: 'all', role: null, lane: null };

/** Campeões que passam em todos os filtros, na ordem recebida (alfabética, do agente). */
export function filterCollection(champions: CollectionChampion[], filters: CollectionFilters): CollectionChampion[] {
  const key = searchKey(filters.query);
  return champions.filter(c =>
    (filters.ownership === 'all' || (filters.ownership === 'owned') === c.owned) &&
    (!filters.role || c.roles.includes(filters.role)) &&
    (!filters.lane || c.positions.includes(filters.lane)) &&
    (!key || searchKey(c.name).includes(key) || searchKey(c.alias).includes(key)));
}

/** Se algum campeão tem rota; sem elas (a LCU não mandou), o filtro de rota some. */
export function hasLanes(champions: CollectionChampion[]): boolean {
  return champions.some(c => c.positions.length > 0);
}

/** Porcentagem inteira de `part` em `total`, 0 sem total. */
export function percent(part: number, total: number): number {
  return total > 0 ? Math.round((part / total) * 100) : 0;
}

const rarityNames: Record<string, string> = {
  epic: 'Épica',
  legendary: 'Lendária',
  mythic: 'Mítica',
  ultimate: 'Suprema',
  transcendent: 'Transcendente',
  exalted: 'Exaltada',
};

/** Nome da raridade da skin, ou `null` quando ela não tem (ou é uma que não conhecemos). */
export function rarityName(rarity: string | null): string | null {
  return rarity ? rarityNames[rarity] ?? null : null;
}

/** Quantas skins (sem a padrão) e cromas da lista a conta tem, e o total. */
export function skinTotals(skins: CollectionSkin[]) {
  const extra = skins.filter(s => !s.isBase);
  const chromas = skins.flatMap(s => s.chromas);
  return {
    ownedSkins: extra.filter(s => s.owned).length,
    totalSkins: extra.length,
    ownedChromas: chromas.filter(c => c.owned).length,
    totalChromas: chromas.length,
  };
}

/** Data em que o campeão foi liberado, como "12 de mar. de 2021". */
export function purchasedLabel(purchased: number | null): string | null {
  if (purchased === null) return null;
  return new Intl.DateTimeFormat('pt-BR', { day: 'numeric', month: 'short', year: 'numeric' }).format(new Date(purchased));
}

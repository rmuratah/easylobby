import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';
import type { RecommendedRunePage } from '../bindings/RecommendedRunePage';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import { LANES, type Lane } from './positions';

/**
 * Campeão do recomendador de runas (#596): o escolhido ou marcado pelo
 * jogador logado, ou a intenção mostrada ao time. `null` antes disso.
 */
export function recommenderChampion(champSelect: ChampSelect): number | null {
  const me = champSelect.myTeam.find(player => player.cellId === champSelect.localPlayerCellId);
  return me?.championId || me?.championPickIntent || null;
}

const isLane = (position: string | undefined): position is Lane => LANES.some(lane => lane === position);

/**
 * Rota em que o recomendador começa: a do jogador na seleção; sem ela
 * (escolha às cegas, preenchimento), a primeira rota do campeão; senão o meio.
 */
export function recommenderLane(champSelect: ChampSelect, catalog: ChampionCatalog | null, championId: number): Lane {
  const me = champSelect.myTeam.find(player => player.cellId === champSelect.localPlayerCellId);
  if (isLane(me?.assignedPosition)) return me.assignedPosition;
  const positions = catalog?.champions.find(champion => champion.id === championId)?.positions ?? [];
  return positions.find(isLane) ?? 'middle';
}

/**
 * Nome da sugestão: o do client oficial ("Executar inimigos e Ouro"); sem
 * ele, a runa principal e a árvore secundária.
 */
export function recommendationTitle(page: RecommendedRunePage, catalog: RuneCatalog | null): string {
  if (page.title) return page.title;
  const keystone = catalog?.perks.find(perk => perk.id === page.selectedPerkIds[0])?.name;
  const sub = catalog?.styles.find(style => style.id === page.subStyleId)?.name;
  return [keystone, sub].filter(Boolean).join(' · ') || 'Página sugerida';
}

/** As runas da sugestão como o client mostra: a principal, as 3 da primária e as 2 da secundária. */
export function recommendationPerks(page: RecommendedRunePage) {
  const ids = page.selectedPerkIds;
  return { keystone: ids[0] ?? 0, primary: ids.slice(1, 4), secondary: ids.slice(4, 6) };
}

import type { ChampSelect } from '../bindings/ChampSelect';
import type { GameQueue } from '../bindings/GameQueue';
import type { Queue } from '../bindings/Queue';
import { isAramChampSelect } from './champSelect';
import type { SummonerSpell } from '../bindings/SummonerSpell';
import type { SummonerSpellCatalog } from '../bindings/SummonerSpellCatalog';

/** Feitiços que valem na fila; com `queue` `null` (personalizada), todos. */
export function spellsForQueue(catalog: SummonerSpellCatalog | null, queue: Queue | null): SummonerSpell[] {
  return (catalog?.spells ?? []).filter(spell => queue === null || spell.queues.includes(queue));
}

/**
 * Modo de jogo da LCU da seleção, para filtrar os feitiços nas filas que não
 * são uma das fixas: o `gameMode` da fila na lista `queues` (ARAM: Desordem é
 * `KIWI`) ou, numa seleção de ARAM fora da lista (personalizada), `ARAM`.
 * `null` quando não dá para saber.
 */
export function champSelectGameMode(champSelect: ChampSelect, queues: GameQueue[]): string | null {
  const listed = queues.find(q => q.queueId === champSelect.queueId);
  if (listed?.gameMode) return listed.gameMode;
  return isAramChampSelect(champSelect) ? 'ARAM' : null;
}

/**
 * Feitiços que valem na seleção (#203): pela fila fixa quando há; senão pelo
 * modo de jogo (`gameModes` de cada feitiço), para a ARAM: Desordem e as
 * personalizadas de ARAM não mostrarem Golpear, Teleporte e os de outros
 * modos. Sem fila nem modo conhecidos, todos.
 */
export function spellsForChampSelect(catalog: SummonerSpellCatalog | null, champSelect: ChampSelect, queues: GameQueue[]): SummonerSpell[] {
  if (champSelect.queue) return spellsForQueue(catalog, champSelect.queue);
  const mode = champSelectGameMode(champSelect, queues);
  if (!mode) return spellsForQueue(catalog, null);
  return (catalog?.spells ?? []).filter(spell => spell.gameModes.includes(mode));
}

/** Se o feitiço já está liberado no nível do jogador (sem nível conhecido, liberado). */
export function spellUnlocked(spell: SummonerSpell, level: number | null | undefined): boolean {
  return level == null || level >= spell.summonerLevel;
}

export type SpellPair = [number, number];

/**
 * Os dois feitiços depois de pôr `spellId` no espaço `slot`. Escolher o que
 * já está no outro espaço troca os dois de lugar, como no client oficial.
 */
export function withSpell([first, second]: SpellPair, slot: 0 | 1, spellId: number): SpellPair {
  if (slot === 0) return spellId === second ? [spellId, first] : [spellId, second];
  return spellId === first ? [second, spellId] : [first, spellId];
}

export function spellName(catalog: SummonerSpellCatalog | null, id: number | null): string {
  if (!id) return 'Nenhum';
  return catalog?.spells.find(spell => spell.id === id)?.name ?? `Feitiço ${id}`;
}

/** Id do Golpear. */
export const SMITE_ID = 11;

/**
 * Regra do Golpear na Solo/Duo e na Flex (#480): o caçador precisa ficar com
 * ele (pode só trocar de tecla) e as outras posições não podem escolher.
 * `null` nas outras filas ou sem posição.
 */
export function smiteRule(champSelect: ChampSelect): 'required' | 'forbidden' | null {
  if (champSelect.queue !== 'rankedSolo' && champSelect.queue !== 'rankedFlex') return null;
  const me = champSelect.myTeam.find(p => p.cellId === champSelect.localPlayerCellId);
  if (!me) return null;
  if (me.assignedPosition === 'jungle') return 'required';
  if (me.assignedPosition === 'unselected' || me.assignedPosition === 'fill') return null;
  return 'forbidden';
}

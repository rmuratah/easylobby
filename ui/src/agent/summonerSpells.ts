import type { Queue } from '../bindings/Queue';
import type { SummonerSpell } from '../bindings/SummonerSpell';
import type { SummonerSpellCatalog } from '../bindings/SummonerSpellCatalog';

/** Feitiços que valem na fila; com `queue` `null` (personalizada), todos. */
export function spellsForQueue(catalog: SummonerSpellCatalog | null, queue: Queue | null): SummonerSpell[] {
  return (catalog?.spells ?? []).filter(spell => queue === null || spell.queues.includes(queue));
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

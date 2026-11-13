import type { ChampSelect } from '../bindings/ChampSelect';
import type { GameQueue } from '../bindings/GameQueue';
import type { Lobby } from '../bindings/Lobby';
import type { Queue } from '../bindings/Queue';
import { isAramChampSelect } from './champSelect';
import type { SummonerSpell } from '../bindings/SummonerSpell';
import type { SummonerSpellCatalog } from '../bindings/SummonerSpellCatalog';

/** Feitiços que valem na fila; com `queue` `null` (personalizada), todos. */
export function spellsForQueue(catalog: SummonerSpellCatalog | null, queue: Queue | null): SummonerSpell[] {
  return (catalog?.spells ?? []).filter(spell => queue === null || spell.queues.includes(queue));
}

/** O lobby da seleção: o mesmo `queueId` (a personalizada inclusive). */
function champSelectLobby(champSelect: ChampSelect, lobby: Lobby | null | undefined): Lobby | null {
  return lobby && lobby.queueId === champSelect.queueId ? lobby : null;
}

/**
 * Modos de jogo da LCU que a seleção pode ter, na ordem de confiança: o
 * `gameMode` do lobby da seleção (cobre a personalizada), o da fila na lista
 * `queues` (ARAM: Desordem é `KIWI`, Arena é `CHERRY`) e, numa seleção de
 * ARAM sem os dois, `ARAM`.
 */
function champSelectGameModes(champSelect: ChampSelect, queues: GameQueue[], lobby?: Lobby | null): string[] {
  const modes = [
    champSelectLobby(champSelect, lobby)?.gameMode,
    queues.find(q => q.queueId === champSelect.queueId)?.gameMode,
    isAramChampSelect(champSelect) ? 'ARAM' : undefined,
  ];
  return modes.filter((mode): mode is string => !!mode);
}

/** Modo de jogo da LCU da seleção, ou `null` quando não dá para saber. */
export function champSelectGameMode(champSelect: ChampSelect, queues: GameQueue[], lobby?: Lobby | null): string | null {
  return champSelectGameModes(champSelect, queues, lobby)[0] ?? null;
}

/**
 * Feitiços que valem na seleção (#203): pela fila fixa quando há; senão pelo
 * modo de jogo (`gameModes` de cada feitiço, como a LCU manda), para cada
 * modo mostrar só os feitiços dele: a personalizada do Summoner's Rift sem
 * Reviver, Fuga e Promover, o ARAM sem Golpear e Teleporte, a Arena só com
 * os dela. Vale o primeiro modo que algum feitiço lista; sem nenhum, todos
 * (a LCU decide).
 */
export function spellsForChampSelect(catalog: SummonerSpellCatalog | null, champSelect: ChampSelect, queues: GameQueue[], lobby?: Lobby | null): SummonerSpell[] {
  if (champSelect.queue) return spellsForQueue(catalog, champSelect.queue);
  const spells = catalog?.spells ?? [];
  for (const mode of champSelectGameModes(champSelect, queues, lobby)) {
    const inMode = spells.filter(spell => spell.gameModes.includes(mode));
    if (inMode.length > 0) return inMode;
  }
  return spells;
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

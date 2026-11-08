import type { Champion } from '../bindings/Champion';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';
import type { Command } from '../bindings/Command';
import type { PlayerSlot } from '../bindings/PlayerSlot';
import type { PlayerSlotInput } from '../bindings/PlayerSlotInput';
import type { Position } from '../bindings/Position';

/** Posições que valem num slot do Jogo Dinâmico (#218), na ordem do client oficial: sem preenchimento. */
export const SLOT_POSITIONS: readonly Position[] = ['top', 'jungle', 'middle', 'bottom', 'utility'];

/** O que o jogador escolheu num slot: a posição e o campeão. `0` e `unselected` enquanto falta escolher. */
export type SlotDraft = { championId: number; position: Position };
export type SlotPair = [SlotDraft, SlotDraft];

const empty: SlotDraft = { championId: 0, position: 'unselected' };

/** Os dois slots confirmados pelo agente, completando com slots vazios. */
export function draftFromSlots(slots: PlayerSlot[]): SlotPair {
  const pick = (slot: PlayerSlot | undefined): SlotDraft => slot
    ? { championId: slot.championId, position: SLOT_POSITIONS.includes(slot.position) ? slot.position : 'unselected' }
    : empty;
  return [pick(slots[0]), pick(slots[1])];
}

function replace(index: 0 | 1, slot: SlotDraft, other: SlotDraft): SlotPair {
  return index === 0 ? [slot, other] : [other, slot];
}

/** Troca a posição de um slot. Se o outro slot já tinha essa posição, os dois trocam, como no client oficial. */
export function withPosition(pair: SlotPair, index: 0 | 1, position: Position): SlotPair {
  const mine = pair[index];
  const other = pair[1 - index];
  const swapped = other.position === position ? { ...other, position: mine.position } : other;
  return replace(index, { ...mine, position }, swapped);
}

/** Troca o campeão de um slot. Se o outro slot já tinha esse campeão, os dois trocam. */
export function withChampion(pair: SlotPair, index: 0 | 1, championId: number): SlotPair {
  const mine = pair[index];
  const other = pair[1 - index];
  const swapped = other.championId === championId ? { ...other, championId: mine.championId } : other;
  return replace(index, { ...mine, championId }, swapped);
}

/** Mesmas regras de check_slots em agent/src/quick_play.rs: duas posições e dois campeões, todos escolhidos e diferentes. */
export function draftComplete([a, b]: SlotPair): boolean {
  return SLOT_POSITIONS.includes(a.position) && SLOT_POSITIONS.includes(b.position) && a.position !== b.position
    && a.championId > 0 && b.championId > 0 && a.championId !== b.championId;
}

export function sameDraft(a: SlotPair, b: SlotPair): boolean {
  return a.every((slot, i) => slot.championId === b[i].championId && slot.position === b[i].position);
}

/** O que mudar além da posição e do campeão num slot: os feitiços ou a página de runas. */
export type SlotExtras = { index: 0 | 1; spells?: [number, number]; runePageId?: number };

/** Comando `setPlayerSlots` com os dois slots. Feitiços e runas em `null` ficam com o que o agente decidir. */
export function slotsCommand(pair: SlotPair, extras?: SlotExtras): Extract<Command, { type: 'setPlayerSlots' }> {
  const slots = pair.map((slot, i): PlayerSlotInput => {
    const mine = extras?.index === i ? extras : undefined;
    return {
      championId: slot.championId,
      position: slot.position,
      spell1Id: mine?.spells?.[0] ?? null,
      spell2Id: mine?.spells?.[1] ?? null,
      runePageId: mine?.runePageId ?? null,
    };
  });
  return { type: 'setPlayerSlots', slots };
}

/** Se a fila pede uma posição prioritária e nenhuma das duas escolhidas é. */
export function missingScarcePosition(pair: SlotPair, scarce: Position[]): boolean {
  return scarce.length > 0 && pair.every(slot => slot.position !== 'unselected')
    && !pair.some(slot => scarce.includes(slot.position));
}

/**
 * Versão de um campeão só de um modo, como a `Jade_Aatrox` (id 60266) do modo Jade: o catálogo traz as duas com o
 * mesmo nome, e no Jogo Dinâmico só vale a normal.
 */
function isModeVariant(champion: Champion): boolean {
  return champion.alias.includes('_');
}

/** Campeões que dá para pôr num slot: os do catálogo que o jogador tem ou pode jogar, sem as versões de modo. Sem a lista do agente, todos. */
export function slotChampions(catalog: ChampionCatalog | null, playable: number[] | null): Champion[] {
  const champions = (catalog?.champions ?? []).filter(champion => !isModeVariant(champion));
  if (!playable) return champions;
  const owned = new Set(playable);
  return champions.filter(champion => owned.has(champion.id));
}

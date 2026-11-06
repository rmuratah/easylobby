import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { RunePage } from '../bindings/RunePage';
import type { RunePageInput } from '../bindings/RunePageInput';
import type { RuneSlot } from '../bindings/RuneSlot';
import type { RuneStyle } from '../bindings/RuneStyle';

/**
 * Rascunho do editor de páginas. As escolhas ficam por linha, porque um
 * fragmento pode aparecer em mais de uma linha (como 5008 em Ofensiva e
 * Flex). `0` é uma linha sem escolha.
 */
export type RuneDraft = {
  name: string;
  primaryStyleId: number;
  subStyleId: number;
  /** Uma runa por linha não-fragmento da primária (principal + 3 comuns). */
  primary: number[];
  /** Até duas escolhas da secundária, cada uma com a linha de onde veio. */
  sub: { row: number; perkId: number }[];
  /** Uma runa por linha de fragmentos da primária. */
  stats: number[];
};

export const primarySlots = (style: RuneStyle | undefined): RuneSlot[] =>
  style?.slots.filter((slot) => slot.kind !== 'statMod') ?? [];
export const subSlots = (style: RuneStyle | undefined): RuneSlot[] =>
  style?.slots.filter((slot) => slot.kind === 'regular') ?? [];
export const statSlots = (style: RuneStyle | undefined): RuneSlot[] =>
  style?.slots.filter((slot) => slot.kind === 'statMod') ?? [];

export function findStyle(catalog: RuneCatalog, id: number): RuneStyle | undefined {
  return catalog.styles.find((style) => style.id === id);
}

/** Árvores que podem ser secundárias da `primary`, como o agente confere. */
export function allowedSubStyles(catalog: RuneCatalog, primary: RuneStyle | undefined): RuneStyle[] {
  if (!primary) return [];
  return catalog.styles.filter((style) => style.id !== primary.id
    && (primary.allowedSubStyles.length === 0 || primary.allowedSubStyles.includes(style.id)));
}

/** Primeira secundária permitida, para quando a atual deixa de valer. */
function firstSubStyle(catalog: RuneCatalog, primary: RuneStyle | undefined): number {
  return allowedSubStyles(catalog, primary)[0]?.id ?? 0;
}

/** Mantém em cada linha a escolha que ainda está nela; o resto vira `0`. */
function keepPerRow(slots: RuneSlot[], picks: number[]): number[] {
  return slots.map((slot, row) => (slot.perks.includes(picks[row]) ? picks[row] : 0));
}

/** Rascunho de uma página nova: primeira árvore e a primeira secundária. */
export function newDraft(catalog: RuneCatalog, name: string): RuneDraft {
  const primary = catalog.styles[0];
  return {
    name,
    primaryStyleId: primary?.id ?? 0,
    subStyleId: firstSubStyle(catalog, primary),
    primary: primarySlots(primary).map(() => 0),
    sub: [],
    stats: statSlots(primary).map(() => 0),
  };
}

/**
 * Rascunho a partir de uma página existente, na ordem de
 * `RunePage.selectedPerkIds`. Runas que não batem com a linha (página
 * incompleta ou de outro patch) ficam sem escolha.
 */
export function draftFromPage(catalog: RuneCatalog, page: RunePage): RuneDraft {
  const primary = findStyle(catalog, page.primaryStyleId);
  const sub = findStyle(catalog, page.subStyleId);
  const pSlots = primarySlots(primary);
  const sSlots = subSlots(sub);
  const stSlots = statSlots(primary);
  const perks = page.selectedPerkIds;
  const subPicks: RuneDraft['sub'] = [];
  for (const perkId of perks.slice(pSlots.length, pSlots.length + 2)) {
    const row = sSlots.findIndex((slot) => slot.perks.includes(perkId));
    if (row >= 0 && !subPicks.some((pick) => pick.row === row)) subPicks.push({ row, perkId });
  }
  return {
    name: page.name,
    primaryStyleId: primary ? primary.id : catalog.styles[0]?.id ?? 0,
    subStyleId: sub && sub.id !== primary?.id ? sub.id : firstSubStyle(catalog, primary ?? catalog.styles[0]),
    primary: keepPerRow(pSlots, perks.slice(0, pSlots.length)),
    sub: subPicks,
    stats: keepPerRow(stSlots, perks.slice(pSlots.length + 2)),
  };
}

/** Troca a primária, mantendo o que ainda vale (fragmentos e secundária). */
export function withPrimary(catalog: RuneCatalog, draft: RuneDraft, styleId: number): RuneDraft {
  if (styleId === draft.primaryStyleId) return draft;
  const primary = findStyle(catalog, styleId);
  const subStillAllowed = allowedSubStyles(catalog, primary).some((style) => style.id === draft.subStyleId);
  return {
    ...draft,
    primaryStyleId: styleId,
    subStyleId: subStillAllowed ? draft.subStyleId : firstSubStyle(catalog, primary),
    primary: primarySlots(primary).map(() => 0),
    sub: subStillAllowed ? draft.sub : [],
    stats: keepPerRow(statSlots(primary), draft.stats),
  };
}

export function withSubStyle(draft: RuneDraft, styleId: number): RuneDraft {
  if (styleId === draft.subStyleId) return draft;
  return { ...draft, subStyleId: styleId, sub: [] };
}

export function withPrimaryPerk(draft: RuneDraft, row: number, perkId: number): RuneDraft {
  return { ...draft, primary: draft.primary.map((pick, i) => (i === row ? perkId : pick)) };
}

export function withStatPerk(draft: RuneDraft, row: number, perkId: number): RuneDraft {
  return { ...draft, stats: draft.stats.map((pick, i) => (i === row ? perkId : pick)) };
}

/**
 * Escolhe uma runa da secundária como o client oficial: na mesma linha de
 * uma escolha, troca essa escolha; numa linha nova com as duas já
 * escolhidas, sai a mais antiga.
 */
export function withSubPerk(draft: RuneDraft, row: number, perkId: number): RuneDraft {
  const others = draft.sub.filter((pick) => pick.row !== row);
  const kept = others.length >= 2 ? others.slice(-1) : others;
  return { ...draft, sub: [...kept, { row, perkId }] };
}

/** O que falta para salvar, ou `null` se a página está pronta. */
export function draftProblem(draft: RuneDraft): string | null {
  if (!draft.name.trim()) return 'Dê um nome à página.';
  if (!draft.primaryStyleId || !draft.subStyleId) return 'Escolha a árvore primária e a secundária.';
  if (draft.primary.some((pick) => !pick)) return 'Escolha uma runa em cada linha da árvore primária.';
  if (draft.sub.length < 2) return 'Escolha duas runas da árvore secundária, em linhas diferentes.';
  if (draft.stats.some((pick) => !pick)) return 'Escolha um fragmento em cada linha.';
  return null;
}

/** Página no formato do comando, na ordem que o agente confere. */
export function draftToInput(draft: RuneDraft): RunePageInput {
  const sub = [...draft.sub].sort((a, b) => a.row - b.row).map((pick) => pick.perkId);
  return {
    name: draft.name.trim(),
    primaryStyleId: draft.primaryStyleId,
    subStyleId: draft.subStyleId,
    selectedPerkIds: [...draft.primary, ...sub, ...draft.stats],
  };
}

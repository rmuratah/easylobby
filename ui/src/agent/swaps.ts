import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectSwap } from '../bindings/ChampSelectSwap';
import type { SwapKind } from '../bindings/SwapKind';

export const swapKinds = ['champion', 'pickOrder', 'position'] as const satisfies readonly SwapKind[];

/** O que se troca, para os textos ("troca de campeão"). */
export const swapKindNames = {
  champion: 'campeão',
  pickOrder: 'vez',
  position: 'posição',
} satisfies Record<SwapKind, string>;

export function swapsOf(champSelect: ChampSelect, kind: SwapKind): ChampSelectSwap[] {
  switch (kind) {
    case 'champion': return champSelect.trades;
    case 'pickOrder': return champSelect.pickOrderSwaps;
    case 'position': return champSelect.positionSwaps;
  }
}

export type SwapEntry = { kind: SwapKind; swap: ChampSelectSwap };

/** Todas as trocas da seleção com um estado, na ordem campeão, vez, posição. */
export function swapsIn(champSelect: ChampSelect, state: ChampSelectSwap['state'], cellId?: number): SwapEntry[] {
  return swapKinds.flatMap(kind => swapsOf(champSelect, kind)
    .filter(swap => swap.state === state && (cellId === undefined || swap.cellId === cellId))
    .map(swap => ({ kind, swap })));
}

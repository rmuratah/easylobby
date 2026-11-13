import type { Position } from '../bindings/Position';

export const positionNames = {
  top: 'Topo',
  jungle: 'Selva',
  middle: 'Meio',
  bottom: 'Atirador',
  utility: 'Suporte',
  fill: 'Preenchimento',
  unselected: 'Não selecionado',
} satisfies Record<Position, string>;

/** As cinco rotas do Summoner's Rift, na ordem em que a tela mostra. */
export const LANES = ['top', 'jungle', 'middle', 'bottom', 'utility'] as const satisfies readonly Position[];

export type Lane = (typeof LANES)[number];

// Mesmas regras de check_positions em agent/src/lobby.rs.
export function validPositions(first: Position, second: Position): boolean {
  return second === 'unselected' || (first !== 'unselected' && first !== 'fill' && first !== second);
}

/** Move o item de `from` para `to`, empurrando os do meio. */
export function moveLane(order: Lane[], from: number, to: number): Lane[] {
  if (from === to || from < 0 || to < 0 || from >= order.length || to >= order.length) return order;
  const next = [...order];
  const [lane] = next.splice(from, 1);
  next.splice(to, 0, lane);
  return next;
}

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

export function isPosition(value: unknown): value is Position {
  return typeof value === 'string' && Object.hasOwn(positionNames, value);
}

// Mesmas regras de check_positions em agent/src/lobby.rs.
export function validPositions(first: Position, second: Position): boolean {
  return second === 'unselected' || (first !== 'unselected' && first !== 'fill' && first !== second);
}

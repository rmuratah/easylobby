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

export function isPosition(value: unknown): value is Position {
  return typeof value === 'string' && Object.hasOwn(positionNames, value);
}

// Mesmas regras de check_positions em agent/src/lobby.rs.
export function validPositions(first: Position, second: Position): boolean {
  return second === 'unselected' || (first !== 'unselected' && first !== 'fill' && first !== second);
}

import { expect, it } from 'vitest';
import { swapsIn, swapsOf } from './swaps';
import { draftChampSelect } from '../test/champSelect';

const champSelect = {
  ...draftChampSelect,
  trades: [{ id: 7, cellId: 1, state: 'RECEIVED' as const }],
  pickOrderSwaps: [{ id: 8, cellId: 1, state: 'AVAILABLE' as const }],
  positionSwaps: [{ id: 9, cellId: 1, state: 'AVAILABLE' as const }, { id: 10, cellId: 2, state: 'SENT' as const }],
};

it('junta as trocas dos três tipos por estado e aliado', () => {
  expect(swapsOf(champSelect, 'pickOrder')).toBe(champSelect.pickOrderSwaps);
  expect(swapsIn(champSelect, 'AVAILABLE').map(e => [e.kind, e.swap.id])).toEqual([['pickOrder', 8], ['position', 9]]);
  expect(swapsIn(champSelect, 'AVAILABLE', 2)).toEqual([]);
  expect(swapsIn(champSelect, 'RECEIVED').map(e => e.kind)).toEqual(['champion']);
  expect(swapsIn(champSelect, 'SENT', 2).map(e => e.swap.id)).toEqual([10]);
});

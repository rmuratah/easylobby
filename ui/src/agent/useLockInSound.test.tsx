import { renderHook } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import type { ChampSelect } from '../bindings/ChampSelect';
import { draftChampSelect } from '../test/champSelect';
import { sound } from './sound';
import { useLockInSound } from './useLockInSound';

afterEach(() => { vi.restoreAllMocks(); });

const confirm = (champSelect: ChampSelect, actionId: number, championId: number): ChampSelect => ({
  ...champSelect,
  actions: champSelect.actions.map(a => a.id === actionId ? { ...a, championId, completed: true, isInProgress: false } : a),
});

it('toca o som do ban e o da escolha quando a LCU confirma, uma vez cada (#498)', () => {
  const play = vi.spyOn(sound, 'playLockIn').mockImplementation(() => {});
  const { rerender } = renderHook(({ cs }) => useLockInSound(cs), { initialProps: { cs: draftChampSelect } });
  expect(play).not.toHaveBeenCalled();
  const banned = confirm(draftChampSelect, 1, 266);
  rerender({ cs: banned });
  expect(play).toHaveBeenCalledWith('ban');
  rerender({ cs: { ...banned } });
  expect(play).toHaveBeenCalledTimes(1);
  rerender({ cs: confirm(banned, 4, 103) });
  expect(play).toHaveBeenLastCalledWith('pick');
  expect(play).toHaveBeenCalledTimes(2);
});

it('o que já estava confirmado ao abrir a tela não toca', () => {
  const play = vi.spyOn(sound, 'playLockIn').mockImplementation(() => {});
  const done = confirm(confirm(draftChampSelect, 1, 266), 4, 103);
  const { rerender } = renderHook(({ cs }) => useLockInSound(cs), { initialProps: { cs: done } });
  rerender({ cs: { ...done } });
  expect(play).not.toHaveBeenCalled();
});

it('ação de outro jogador não toca', () => {
  const play = vi.spyOn(sound, 'playLockIn').mockImplementation(() => {});
  const other = draftChampSelect.actions.find(a => a.actorCellId !== draftChampSelect.localPlayerCellId)!;
  const { rerender } = renderHook(({ cs }) => useLockInSound(cs), { initialProps: { cs: draftChampSelect } });
  rerender({ cs: confirm(draftChampSelect, other.id, 62) });
  expect(play).not.toHaveBeenCalled();
});

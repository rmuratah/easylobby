import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ERROR_DISMISS_MS, useDismiss } from './useDismiss';

beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); });

it('limpa o erro depois do tempo e recomeça a contagem com uma mensagem nova', () => {
  const clear = vi.fn();
  const { rerender } = renderHook(({ active, key }) => useDismiss(active, key, clear), { initialProps: { active: true, key: 'a' } });
  act(() => { vi.advanceTimersByTime(ERROR_DISMISS_MS - 100); });
  rerender({ active: true, key: 'b' });
  act(() => { vi.advanceTimersByTime(ERROR_DISMISS_MS - 100); });
  expect(clear).not.toHaveBeenCalled();
  act(() => { vi.advanceTimersByTime(100); });
  expect(clear).toHaveBeenCalledTimes(1);
});

it('não limpa nada se o erro saiu antes', () => {
  const clear = vi.fn();
  const { rerender } = renderHook(({ active }) => useDismiss(active, 'a', clear), { initialProps: { active: true } });
  rerender({ active: false });
  act(() => { vi.advanceTimersByTime(ERROR_DISMISS_MS * 2); });
  expect(clear).not.toHaveBeenCalled();
});

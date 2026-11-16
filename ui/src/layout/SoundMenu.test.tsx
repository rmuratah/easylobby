import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { sound } from '../agent/sound';
import { SoundMenu } from './SoundMenu';

afterEach(() => vi.useRealTimers());

it('liga e desliga o som e muda o volume, com som de teste', () => {
  vi.useFakeTimers();
  const play = vi.spyOn(sound, 'play').mockImplementation(() => {});
  const onChange = vi.fn();
  render(<SoundMenu settings={{ enabled: true, volume: 0.7 }} onChange={onChange} icon={null} />);
  fireEvent.click(screen.getByRole('button', { name: 'Configurações' }));
  fireEvent.click(screen.getByRole('switch', { name: 'Sons do app' }));
  expect(onChange).toHaveBeenLastCalledWith({ enabled: false, volume: 0.7 });
  fireEvent.change(screen.getByRole('slider', { name: /Volume/ }), { target: { value: '40' } });
  expect(onChange).toHaveBeenLastCalledWith({ enabled: true, volume: 0.4 });
  act(() => { vi.runAllTimers(); });
  expect(play).toHaveBeenCalledWith('matchFound');
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(screen.queryByRole('dialog', { name: 'Configurações' })).toBeNull();
});

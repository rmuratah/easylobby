import { expect, it, vi } from 'vitest';
import { createSoundPlayer } from './sound';

function fakeContext(state: AudioContextState) {
  const param = () => ({ value: 0, setValueAtTime: vi.fn(), exponentialRampToValueAtTime: vi.fn() });
  const node = () => ({ connect: vi.fn((next: unknown) => next) });
  return {
    state, currentTime: 10, destination: {},
    resume: vi.fn(() => Promise.resolve()),
    createOscillator: vi.fn(() => ({ ...node(), type: '', frequency: param(), start: vi.fn(), stop: vi.fn() })),
    createGain: vi.fn(() => ({ ...node(), gain: param() })),
  };
}

it('toca três notas e libera o áudio suspenso', () => {
  const ctx = fakeContext('suspended');
  const create = vi.fn(() => ctx as unknown as AudioContext);
  const player = createSoundPlayer(create);
  player.unlock();
  expect(ctx.resume).toHaveBeenCalledTimes(1);
  player.playMatchFound();
  expect(create).toHaveBeenCalledTimes(1);
  expect(ctx.createOscillator).toHaveBeenCalledTimes(3);
  const first = ctx.createOscillator.mock.results[0].value;
  expect(first.start).toHaveBeenCalledWith(10.05);
});

it('não quebra sem Web Audio', () => {
  const player = createSoundPlayer(() => { throw new Error('sem áudio'); });
  expect(() => { player.unlock(); player.playMatchFound(); }).not.toThrow();
  expect(() => createSoundPlayer(() => null).playMatchFound()).not.toThrow();
});

it('toca o tique da contagem, mais agudo nos últimos segundos', () => {
  const ctx = fakeContext('running');
  const player = createSoundPlayer(() => ctx as unknown as AudioContext);
  player.playTick();
  player.playTick(true);
  expect(ctx.createOscillator).toHaveBeenCalledTimes(2);
  const [normal, urgent] = ctx.createOscillator.mock.results.map(r => r.value);
  expect(urgent.frequency.value).toBeGreaterThan(normal.frequency.value);
});

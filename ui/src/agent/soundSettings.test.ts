import { afterEach, expect, it } from 'vitest';
import { readSoundSettings } from './soundSettings';

afterEach(() => localStorage.clear());

it('começa com o som ligado em 70%', () => {
  expect(readSoundSettings()).toEqual({ enabled: true, volume: 0.7 });
});

it('lê a escolha salva e ignora valores inválidos', () => {
  localStorage.setItem('easylobby.som', JSON.stringify({ enabled: false, volume: 0.4 }));
  expect(readSoundSettings()).toEqual({ enabled: false, volume: 0.4 });
  localStorage.setItem('easylobby.som', JSON.stringify({ enabled: 'sim', volume: 7 }));
  expect(readSoundSettings()).toEqual({ enabled: true, volume: 1 });
  localStorage.setItem('easylobby.som', '{quebrado');
  expect(readSoundSettings()).toEqual({ enabled: true, volume: 0.7 });
});

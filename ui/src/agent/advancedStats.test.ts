import { expect, it } from 'vitest';
import { runeResults } from './advancedStats';
import type { AdvancedStats } from '../bindings/AdvancedStats';
import { testGameResult } from '../test/gameResult';

const stats: AdvancedStats = { ...testGameResult.teams[0]!.advanced[0]!, perks: [{ id: 8304, vars: [8, 1, 5] }, { id: 8437, vars: [1116, 635, 0] }] };

it('troca as variáveis das runas pelos valores, com os tempos montados como a Riot manda', () => {
  const perks = [
    { id: 8304, name: 'Calçados Mágicos', shortDesc: '', longDesc: '', endOfGameStatDescs: ['Tempo de chegada das botas: @eogvar1@:@eogvar2@@eogvar3@'] },
    { id: 8437, name: 'Aperto dos Mortos-Vivos', shortDesc: '', longDesc: '', endOfGameStatDescs: ['Total de dano: @eogvar1@', 'Total de cura: @eogvar2@'] },
  ];
  expect(runeResults(stats, perks)).toEqual([
    { id: 8304, name: 'Calçados Mágicos', lines: ['Tempo de chegada das botas: 8:15'] },
    { id: 8437, name: 'Aperto dos Mortos-Vivos', lines: ['Total de dano: 1.116', 'Total de cura: 635'] },
  ]);
  // Sem o catálogo: só o id.
  expect(runeResults(stats, [])[0]).toEqual({ id: 8304, name: '', lines: [] });
});

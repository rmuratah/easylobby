import { describe, expect, it } from 'vitest';
import { spellName, spellsForQueue, spellUnlocked, withSpell } from './summonerSpells';
import { spellCatalog } from '../test/champSelect';

describe('feitiços de invocador', () => {
  it('filtra pela fila, e mostra todos sem fila', () => {
    expect(spellsForQueue(spellCatalog, 'rankedSolo').map(s => s.name)).toEqual(['Flash', 'Incendiar', 'Golpear', 'Barreira']);
    expect(spellsForQueue(spellCatalog, 'aram').map(s => s.name)).toEqual(['Flash', 'Incendiar', 'Marcar', 'Barreira']);
    expect(spellsForQueue(spellCatalog, null)).toHaveLength(5);
    expect(spellsForQueue(null, 'aram')).toEqual([]);
  });
  it('confere o nível', () => {
    const smite = spellCatalog.spells[2];
    expect(spellUnlocked(smite, 8)).toBe(false);
    expect(spellUnlocked(smite, 9)).toBe(true);
    expect(spellUnlocked(smite, null)).toBe(true);
  });
  it('troca de lugar ao escolher o feitiço do outro espaço', () => {
    expect(withSpell([4, 14], 0, 11)).toEqual([11, 14]);
    expect(withSpell([4, 14], 1, 11)).toEqual([4, 11]);
    expect(withSpell([4, 14], 0, 14)).toEqual([14, 4]);
    expect(withSpell([4, 14], 1, 4)).toEqual([14, 4]);
  });
  it('nomeia o feitiço, com nome provisório sem o catálogo', () => {
    expect(spellName(spellCatalog, 4)).toBe('Flash');
    expect(spellName(null, 4)).toBe('Feitiço 4');
    expect(spellName(spellCatalog, null)).toBe('Nenhum');
  });
});

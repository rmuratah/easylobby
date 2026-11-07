import { describe, expect, it } from 'vitest';
import { spellName, spellsForChampSelect, spellsForQueue, spellUnlocked, withSpell } from './summonerSpells';
import { aramChampSelect, draftChampSelect, spellCatalog } from '../test/champSelect';
import type { GameQueue } from '../bindings/GameQueue';
import type { SummonerSpellCatalog } from '../bindings/SummonerSpellCatalog';

describe('feitiços de invocador', () => {
  it('filtra pela fila, e mostra todos sem fila', () => {
    expect(spellsForQueue(spellCatalog, 'rankedSolo').map(s => s.name)).toEqual(['Flash', 'Incendiar', 'Golpear', 'Barreira']);
    expect(spellsForQueue(spellCatalog, 'aram').map(s => s.name)).toEqual(['Flash', 'Incendiar', 'Marcar', 'Barreira']);
    expect(spellsForQueue(spellCatalog, null)).toHaveLength(5);
    expect(spellsForQueue(null, 'aram')).toEqual([]);
  });
  it('filtra pelo modo de jogo na ARAM: Desordem e na personalizada de ARAM (#203)', () => {
    const spell = (id: number, name: string, gameModes: string[]) => ({ id, name, description: '', summonerLevel: 1, gameModes, queues: [] });
    const catalog: SummonerSpellCatalog = { spells: [
      spell(4, 'Flash', ['CLASSIC', 'ARAM', 'KIWI']), spell(3, 'Exaustão', ['CLASSIC', 'ARAM']),
      spell(11, 'Golpear', ['CLASSIC']), spell(32, 'Marca', ['ARAM', 'KIWI']), spell(714, 'Incendiar', ['JADE']),
    ] };
    const mayhem: GameQueue = { queueId: 2400, queue: null, name: 'ARAM: Desordem', description: '', gameMode: 'KIWI',
      modeGroup: 'kARAM', mapId: 12, versusAi: false, ranked: false, showPositionSelector: false, limitedTime: false, available: true };
    const names = (list: { name: string }[]) => list.map(s => s.name);
    expect(names(spellsForChampSelect(catalog, { ...aramChampSelect, queueId: 2400, queue: null }, [mayhem]))).toEqual(['Flash', 'Marca']);
    // Personalizada de ARAM (fila fora da lista, com banco): modo ARAM.
    expect(names(spellsForChampSelect(catalog, { ...aramChampSelect, queueId: 3270, queue: null }, []))).toEqual(['Flash', 'Exaustão', 'Marca']);
    // Personalizada que não é ARAM: todos, a LCU decide.
    expect(spellsForChampSelect(catalog, { ...draftChampSelect, queueId: 0, queue: null }, [])).toHaveLength(5);
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

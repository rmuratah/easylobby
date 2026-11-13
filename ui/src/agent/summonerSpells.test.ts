import { describe, expect, it } from 'vitest';
import { spellName, spellsForChampSelect, spellsForQueue, spellUnlocked, withSpell } from './summonerSpells';
import { aramChampSelect, draftChampSelect, spellCatalog } from '../test/champSelect';
import type { GameQueue } from '../bindings/GameQueue';
import { testLobby } from '../test/lobby';
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
    // Personalizada que não é ARAM e sem lobby: todos, a LCU decide.
    expect(spellsForChampSelect(catalog, { ...draftChampSelect, queueId: 0, queue: null }, [])).toHaveLength(5);
  });
  it('filtra pelo modo do lobby na personalizada e em qualquer modo', () => {
    const spell = (id: number, name: string, gameModes: string[]) => ({ id, name, description: '', summonerLevel: 1, gameModes, queues: [] });
    const catalog: SummonerSpellCatalog = { spells: [
      spell(4, 'Flash', ['CLASSIC', 'ARAM']), spell(11, 'Golpear', ['CLASSIC']), spell(32, 'Marca', ['ARAM']),
      spell(777, 'Reviver', ['JADE']), spell(716, 'Surto', ['JADE']), spell(2201, 'Fuga', ['CHERRY']), spell(2202, 'Flash', ['CHERRY']),
    ] };
    const names = (list: { name: string }[]) => list.map(s => s.name);
    const custom = { ...draftChampSelect, queueId: 0, queue: null, isCustomGame: true };
    const lobby = (gameMode: string, queueId = 0) => ({ ...testLobby, queueId, queue: null, gameMode });
    // Personalizada alternada no Summoner's Rift: sem Reviver, Surto e Fuga.
    expect(names(spellsForChampSelect(catalog, custom, [], lobby('CLASSIC')))).toEqual(['Flash', 'Golpear']);
    expect(names(spellsForChampSelect(catalog, custom, [], lobby('ARAM')))).toEqual(['Flash', 'Marca']);
    expect(names(spellsForChampSelect(catalog, custom, [], lobby('CHERRY')))).toEqual(['Fuga', 'Flash']);
    expect(names(spellsForChampSelect(catalog, custom, [], lobby('JADE')))).toEqual(['Reviver', 'Surto']);
    // Lobby de outra fila não vale.
    expect(spellsForChampSelect(catalog, custom, [], lobby('CLASSIC', 420))).toHaveLength(7);
    // Modo que nenhum feitiço lista: todos.
    expect(spellsForChampSelect(catalog, custom, [], lobby('NOVO'))).toHaveLength(7);
    // Arena pela lista de filas.
    const arena: GameQueue = { queueId: 1700, queue: null, name: 'Arena', description: '', gameMode: 'CHERRY',
      modeGroup: 'kCherry', mapId: 30, versusAi: false, ranked: false, showPositionSelector: false, limitedTime: false, available: true };
    expect(names(spellsForChampSelect(catalog, { ...draftChampSelect, queueId: 1700, queue: null }, [arena]))).toEqual(['Fuga', 'Flash']);
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

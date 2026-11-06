import { describe, expect, it } from 'vitest';
import {
  actingCells, championName, gridChampions, gridPurpose, playerName, remainingMs, searchChampions, searchKey,
} from './champSelect';
import { testChampionCatalog } from '../test/events';
import { champSelectState, draftChampSelect } from '../test/champSelect';

const names = (list: { name: string }[]) => list.map(c => c.name);

describe('busca de campeões', () => {
  it('ignora acento, maiúscula, espaço e pontuação', () => {
    expect(searchKey("Kai'Sa")).toBe('kaisa');
    expect(searchKey('Nunu É Willump')).toBe('nunuewillump');
    expect(names(searchChampions(testChampionCatalog.champions, 'kai sa'))).toEqual(["Kai'Sa"]);
    expect(names(searchChampions(testChampionCatalog.champions, 'ÁH'))).toEqual(['Ahri']);
  });
  it('acha pelo nome interno em inglês e devolve tudo sem busca', () => {
    expect(names(searchChampions(testChampionCatalog.champions, 'monkey'))).toEqual(['Wukong']);
    expect(searchChampions(testChampionCatalog.champions, '  ')).toHaveLength(testChampionCatalog.champions.length);
    expect(searchChampions(testChampionCatalog.champions, 'zzz')).toEqual([]);
  });
});

describe('grade', () => {
  it('na vez do ban mostra os que dá para banir, com os escolhidos indisponíveis', () => {
    expect(gridPurpose(draftChampSelect)).toBe('ban');
    const grid = gridChampions(champSelectState, draftChampSelect);
    expect(grid.map(g => [g.champion.id, g.available])).toEqual([[266, true], [103, false], [145, true], [20, true], [62, true]]);
  });
  it('fora do ban mostra os que dá para escolher, sem os banidos', () => {
    const champSelect = { ...draftChampSelect, pendingAction: null, bans: { myTeam: [266], theirTeam: [] } };
    expect(gridPurpose(champSelect)).toBe('pick');
    const grid = gridChampions(champSelectState, champSelect);
    expect(grid.map(g => [g.champion.id, g.available])).toEqual([[266, false], [103, false], [20, true], [62, true]]);
  });
  it('sem a lista da LCU usa o catálogo inteiro; sem catálogo fica vazia', () => {
    const unknown = { ...champSelectState, champSelectChampions: { pickable: null, bannable: null } };
    expect(gridChampions(unknown, draftChampSelect)).toHaveLength(testChampionCatalog.champions.length);
    expect(gridChampions({ ...unknown, championCatalog: null }, draftChampSelect)).toEqual([]);
  });
  it('não marca como indisponível o campeão do próprio jogador', () => {
    const champSelect = {
      ...draftChampSelect, pendingAction: null,
      actions: [{ id: 9, type: 'pick' as const, actorCellId: 0, championId: 62, completed: true, isInProgress: false, isAllyAction: true, turn: 1 }],
    };
    expect(gridChampions(champSelectState, champSelect).find(g => g.champion.id === 62)?.available).toBe(true);
  });
});

describe('textos da seleção', () => {
  it('nomeia campeões e jogadores, com nome provisório quando falta', () => {
    expect(championName(testChampionCatalog, 266)).toBe('Aatrox');
    expect(championName(null, 266)).toBe('Campeão 266');
    expect(playerName(draftChampSelect.myTeam[0], 0, true)).toBe('Victor#BR1');
    expect(playerName(draftChampSelect.theirTeam[0], 0, false)).toBe('Adversário 1');
    expect(playerName({ ...draftChampSelect.myTeam[0], gameName: null }, 2, true)).toBe('Aliado 3');
  });
  it('calcula o tempo restante sem ficar negativo e quem está na vez', () => {
    const end = draftChampSelect.timer.endsAtMs;
    expect(remainingMs(draftChampSelect, end - 1500)).toBe(1500);
    expect(remainingMs(draftChampSelect, end + 1)).toBe(0);
    expect([...actingCells(draftChampSelect)]).toEqual([0, 1]);
  });
});

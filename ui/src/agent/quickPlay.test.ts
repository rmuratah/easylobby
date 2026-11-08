import { describe, expect, it } from 'vitest';
import type { PlayerSlot } from '../bindings/PlayerSlot';
import {
  draftComplete, draftFromSlots, missingScarcePosition, sameDraft, slotChampions, slotsCommand, withChampion, withPosition,
  type SlotPair,
} from './quickPlay';

const slot = (championId: number, position: PlayerSlot['position']): PlayerSlot => ({
  championId, skinId: 0, position, spell1Id: 4, spell2Id: 14, runeStyleId: 0, runeSubStyleId: 0, keystoneId: 0,
});

describe('slots do Jogo Dinâmico', () => {
  it('completa os slots que faltam e ignora posição que não vale', () => {
    expect(draftFromSlots([])).toEqual([{ championId: 0, position: 'unselected' }, { championId: 0, position: 'unselected' }]);
    expect(draftFromSlots([slot(22, 'bottom'), slot(266, 'fill')])).toEqual([
      { championId: 22, position: 'bottom' }, { championId: 266, position: 'unselected' }]);
  });

  it('troca posição e campeão entre os slots quando repetem', () => {
    const pair: SlotPair = [{ championId: 22, position: 'bottom' }, { championId: 266, position: 'top' }];
    expect(withPosition(pair, 0, 'top')).toEqual([{ championId: 22, position: 'top' }, { championId: 266, position: 'bottom' }]);
    expect(withPosition(pair, 1, 'jungle')).toEqual([pair[0], { championId: 266, position: 'jungle' }]);
    expect(withChampion(pair, 1, 22)).toEqual([{ championId: 266, position: 'bottom' }, { championId: 22, position: 'top' }]);
  });

  it('só fica completo com tudo escolhido e diferente', () => {
    expect(draftComplete([{ championId: 22, position: 'bottom' }, { championId: 266, position: 'top' }])).toBe(true);
    expect(draftComplete([{ championId: 22, position: 'bottom' }, { championId: 0, position: 'top' }])).toBe(false);
    expect(draftComplete([{ championId: 22, position: 'bottom' }, { championId: 266, position: 'unselected' }])).toBe(false);
    expect(draftComplete([{ championId: 22, position: 'fill' }, { championId: 266, position: 'top' }])).toBe(false);
    expect(sameDraft(draftFromSlots([slot(22, 'bottom')]), draftFromSlots([slot(22, 'bottom')]))).toBe(true);
  });

  it('monta o comando com feitiços e runas só no slot que mudou', () => {
    const pair: SlotPair = [{ championId: 22, position: 'bottom' }, { championId: 266, position: 'top' }];
    expect(slotsCommand(pair, { index: 1, spells: [4, 12] })).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null },
      { championId: 266, position: 'top', spell1Id: 4, spell2Id: 12, runePageId: null },
    ] });
    expect(slotsCommand(pair, { index: 0, runePageId: 7 }).slots[0].runePageId).toBe(7);
  });

  it('avisa quando nenhuma posição escolhida é prioritária', () => {
    const pair: SlotPair = [{ championId: 22, position: 'middle' }, { championId: 266, position: 'top' }];
    expect(missingScarcePosition(pair, ['jungle', 'bottom'])).toBe(true);
    expect(missingScarcePosition(withPosition(pair, 1, 'jungle'), ['jungle', 'bottom'])).toBe(false);
    expect(missingScarcePosition(pair, [])).toBe(false);
  });

  it('filtra os campeões pelo que o jogador pode jogar', () => {
    const catalog = { champions: [
      { id: 266, name: 'Aatrox', alias: 'Aatrox', roles: [], positions: [] }, { id: 22, name: 'Ashe', alias: 'Ashe', roles: [], positions: [] }] };
    expect(slotChampions(catalog, [22]).map(c => c.id)).toEqual([22]);
    expect(slotChampions(catalog, null).map(c => c.id)).toEqual([266, 22]);
    expect(slotChampions(null, [22])).toEqual([]);
  });

  it('esconde as versões de modo do mesmo campeão (#218)', () => {
    const catalog = { champions: [
      { id: 266, name: 'Aatrox', alias: 'Aatrox', roles: [], positions: [] }, { id: 60266, name: 'Aatrox', alias: 'Jade_Aatrox', roles: [], positions: [] }] };
    expect(slotChampions(catalog, [266, 60266]).map(c => c.id)).toEqual([266]);
    expect(slotChampions(catalog, null).map(c => c.id)).toEqual([266]);
  });
});

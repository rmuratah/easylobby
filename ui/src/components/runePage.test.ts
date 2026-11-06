import { describe, expect, it } from 'vitest';
import { testFullRuneCatalog as catalog, testRunePage } from '../test/runes';
import {
  draftFromPage, draftProblem, draftToInput, newDraft, withPrimary, withPrimaryPerk, withStatPerk, withSubPerk, withSubStyle,
} from './runePage';

describe('rascunho da página de runas', () => {
  it('lê uma página e devolve as runas na ordem que o agente confere', () => {
    const draft = draftFromPage(catalog, testRunePage);
    expect(draftProblem(draft)).toBeNull();
    expect(draftToInput(draft)).toEqual({
      name: 'Mid', primaryStyleId: 8000, subStyleId: 8100, selectedPerkIds: testRunePage.selectedPerkIds,
    });
  });

  it('página nova começa vazia e diz o que falta', () => {
    let draft = newDraft(catalog, ' ');
    expect(draft).toMatchObject({ primaryStyleId: 8000, subStyleId: 8100, primary: [0, 0, 0, 0], sub: [], stats: [0, 0, 0] });
    expect(draftProblem(draft)).toMatch(/nome/);
    draft = { ...draft, name: 'Nova' };
    expect(draftProblem(draft)).toMatch(/primária/);
    [101, 112, 122, 132].forEach((perk, row) => { draft = withPrimaryPerk(draft, row, perk); });
    expect(draftProblem(draft)).toMatch(/secundária/);
    draft = withSubPerk(withSubPerk(draft, 2, 232), 0, 212);
    expect(draftProblem(draft)).toMatch(/fragmento/);
    [5005, 5008, 5001].forEach((perk, row) => { draft = withStatPerk(draft, row, perk); });
    expect(draftProblem(draft)).toBeNull();
    // A secundária sai ordenada pela linha, não pela ordem do clique.
    expect(draftToInput(draft).selectedPerkIds).toEqual([101, 112, 122, 132, 212, 232, 5005, 5008, 5001]);
  });

  it('secundária troca na mesma linha e tira a mais antiga numa linha nova', () => {
    let draft = withSubPerk(withSubPerk(newDraft(catalog, 'x'), 0, 211), 1, 221);
    draft = withSubPerk(draft, 1, 222);
    expect(draft.sub).toEqual([{ row: 0, perkId: 211 }, { row: 1, perkId: 222 }]);
    draft = withSubPerk(draft, 2, 231);
    expect(draft.sub).toEqual([{ row: 1, perkId: 222 }, { row: 2, perkId: 231 }]);
  });

  it('trocar a primária mantém fragmentos e a secundária que ainda vale', () => {
    const draft = draftFromPage(catalog, testRunePage);
    const sorcery = withPrimary(catalog, draft, 8200);
    expect(sorcery).toMatchObject({ primaryStyleId: 8200, subStyleId: 8000, primary: [0, 0, 0, 0], sub: [], stats: [5008, 5010, 5011] });
    const domination = withPrimary(catalog, draft, 8100);
    expect(domination).toMatchObject({ subStyleId: 8000, sub: [] });
    const toPrecision = withPrimary(catalog, { ...draft, primaryStyleId: 8100, subStyleId: 8200 }, 8000);
    expect(toPrecision.subStyleId).toBe(8200);
    expect(withSubStyle(draft, 8200).sub).toEqual([]);
  });

  it('runas fora do lugar ficam sem escolha', () => {
    const draft = draftFromPage(catalog, { ...testRunePage, selectedPerkIds: [101, 0, 121, 999, 211, 212, 5008] });
    expect(draft.primary).toEqual([101, 0, 121, 0]);
    expect(draft.sub).toEqual([{ row: 0, perkId: 211 }]);
    expect(draft.stats).toEqual([5008, 0, 0]);
  });
});

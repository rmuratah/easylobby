import { describe, expect, it } from 'vitest';
import type { CollectionChampion } from '../bindings/CollectionChampion';
import { emptyFilters, filterCollection, hasLanes, percent, rarityName, skinTotals } from './collection';

function champion(id: number, name: string, change: Partial<CollectionChampion> = {}): CollectionChampion {
  return {
    id, name, alias: name, title: '', roles: [], positions: [], owned: true, freeToPlay: false, rented: false,
    purchased: null, ownedSkins: 0, totalSkins: 0, ...change,
  };
}

const champions = [
  champion(266, 'Aatrox', { roles: ['fighter', 'tank'], positions: ['top'] }),
  champion(103, 'Ahri', { roles: ['mage', 'assassin'], positions: ['middle'], owned: false, freeToPlay: true }),
  champion(62, 'Wukong', { alias: 'MonkeyKing', roles: ['fighter'], positions: ['top', 'jungle'], owned: false }),
  champion(145, "Kai'Sa", { alias: 'Kaisa', roles: ['marksman'], positions: ['bottom'] }),
];
const names = (list: CollectionChampion[]) => list.map(c => c.name);

describe('filtros da Coleção (#369)', () => {
  it('sem filtro, mostra todos na ordem recebida', () => {
    expect(names(filterCollection(champions, emptyFilters))).toEqual(['Aatrox', 'Ahri', 'Wukong', "Kai'Sa"]);
  });

  it('separa os da conta dos que faltam', () => {
    expect(names(filterCollection(champions, { ...emptyFilters, ownership: 'owned' }))).toEqual(['Aatrox', "Kai'Sa"]);
    expect(names(filterCollection(champions, { ...emptyFilters, ownership: 'missing' }))).toEqual(['Ahri', 'Wukong']);
  });

  it('busca pelo nome sem acento e pontuação, e pelo nome em inglês', () => {
    expect(names(filterCollection(champions, { ...emptyFilters, query: 'kais' }))).toEqual(["Kai'Sa"]);
    expect(names(filterCollection(champions, { ...emptyFilters, query: 'monkey' }))).toEqual(['Wukong']);
  });

  it('junta função, rota e conta', () => {
    expect(names(filterCollection(champions, { ...emptyFilters, role: 'fighter' }))).toEqual(['Aatrox', 'Wukong']);
    expect(names(filterCollection(champions, { ...emptyFilters, role: 'fighter', lane: 'jungle' }))).toEqual(['Wukong']);
    expect(names(filterCollection(champions, { ...emptyFilters, role: 'fighter', ownership: 'owned' }))).toEqual(['Aatrox']);
  });

  it('esconde o filtro de rota sem rotas', () => {
    expect(hasLanes(champions)).toBe(true);
    expect(hasLanes([champion(1, 'Annie')])).toBe(false);
  });
});

describe('números da Coleção', () => {
  it('porcentagem sem dividir por zero', () => {
    expect(percent(1, 3)).toBe(33);
    expect(percent(0, 0)).toBe(0);
  });

  it('raridade com nome em português', () => {
    expect(rarityName('legendary')).toBe('Lendária');
    expect(rarityName(null)).toBeNull();
    expect(rarityName('desconhecida')).toBeNull();
  });

  it('conta skins sem a padrão, e os cromas', () => {
    expect(skinTotals([
      { id: 1000, name: 'Padrão', isBase: true, owned: true, rarity: null, chromas: [] },
      { id: 1001, name: 'A', isBase: false, owned: true, rarity: null,
        chromas: [{ id: 1020, name: 'Rubi', owned: true, colors: [] }, { id: 1021, name: 'Safira', owned: false, colors: [] }] },
      { id: 1002, name: 'B', isBase: false, owned: false, rarity: 'epic', chromas: [] },
    ])).toEqual({ ownedSkins: 1, totalSkins: 2, ownedChromas: 1, totalChromas: 2 });
  });
});

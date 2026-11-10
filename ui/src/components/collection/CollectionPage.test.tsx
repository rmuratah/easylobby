import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { CollectionPage } from './CollectionPage';
import { makeState } from '../../test/state';
import type { AgentState } from '../../bindings/AgentState';
import type { Collection } from '../../bindings/Collection';
import type { ChampionSkins } from '../../bindings/ChampionSkins';
import type { ConnectionSnapshot } from '../../agent/client';

const summoner = {
  gameName: 'Jogador', tagLine: 'BR1', displayName: '', summonerLevel: 87, profileIconId: 29,
  puuid: 'fake-puuid-0001', summonerId: 1,
};

const collection: Collection = {
  champions: [
    { id: 266, name: 'Aatrox', alias: 'Aatrox', title: 'a Espada Darkin', roles: ['fighter', 'tank'], positions: ['top'],
      owned: true, freeToPlay: false, rented: false, purchased: Date.UTC(2021, 2, 12, 12), ownedSkins: 1, totalSkins: 2 },
    { id: 103, name: 'Ahri', alias: 'Ahri', title: '', roles: ['mage'], positions: ['middle'],
      owned: false, freeToPlay: true, rented: false, purchased: null, ownedSkins: 0, totalSkins: 1 },
    { id: 62, name: 'Wukong', alias: 'MonkeyKing', title: '', roles: ['fighter'], positions: ['jungle'],
      owned: false, freeToPlay: false, rented: false, purchased: null, ownedSkins: 0, totalSkins: 0 },
  ],
  ownedChampions: 1, totalChampions: 3, ownedSkins: 1, totalSkins: 3,
};

const aatroxSkins: ChampionSkins = {
  championId: 266,
  skins: [
    { id: 266000, name: 'Aatrox', isBase: true, owned: true, rarity: null, chromas: [] },
    { id: 266001, name: 'Aatrox Justiceiro', isBase: false, owned: true, rarity: 'epic', chromas: [] },
    { id: 266002, name: 'Aatrox Lua de Sangue', isBase: false, owned: false, rarity: 'legendary', chromas: [
      { id: 266010, name: 'Rubi', owned: true, colors: ['#c82b2b', '#2b5dc8'] },
      { id: 266011, name: 'Safira', owned: false, colors: [] },
    ] },
  ],
};

function setup(overrides: Partial<AgentState> = {}) {
  const send = vi.fn(() => Promise.resolve({ status: 'ok' as const }));
  const snapshot = (next: Partial<AgentState>): ConnectionSnapshot =>
    ({ status: 'connected', error: null, state: makeState({ summoner, ...overrides, ...next }) });
  const view = render(<CollectionPage connection={snapshot({})} send={send} />);
  return { send, rerender: (next: Partial<AgentState>) => view.rerender(<CollectionPage connection={snapshot(next)} send={send} />) };
}

describe('CollectionPage (#369)', () => {
  it('sem invocador, explica o que fazer e não pede nada', () => {
    const send = vi.fn();
    render(<CollectionPage connection={{ status: 'connected', error: null, state: makeState() }} send={send} />);
    expect(screen.getByText('Coleção indisponível')).toBeInTheDocument();
    expect(send).not.toHaveBeenCalled();
  });

  it('pede a coleção ao abrir e mostra o carregamento', () => {
    const { send } = setup();
    expect(send).toHaveBeenCalledWith({ type: 'openCollection' });
    expect(screen.getByText('Carregando a coleção…')).toBeInTheDocument();
  });

  it('mostra as contagens e a grade, e filtra', () => {
    setup({ collection });
    expect(screen.getByRole('progressbar', { name: 'Campeões: 1 de 3' })).toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: 'Skins: 1 de 3' })).toBeInTheDocument();
    const grid = screen.getByRole('list', { name: 'Grade de campeões' });
    expect(within(grid).getAllByRole('button')).toHaveLength(3);
    expect(within(grid).getByRole('button', { name: 'Ahri, na rotação gratuita, 0 de 1 skins' })).toHaveTextContent('Grátis');
    // Card em pé com a tela de carregamento da skin padrão (#394).
    const aatrox = within(grid).getByRole('button', { name: /^Aatrox, na conta/ });
    expect([...aatrox.querySelectorAll('img')].map(img => img.getAttribute('src'))).toContainEqual(expect.stringMatching(/\/assets\/skin\/266000$/));

    fireEvent.click(screen.getByRole('button', { name: 'Não tenho' }));
    expect(within(grid).getAllByRole('button').map(b => b.textContent)).toEqual(['Grátis0/1 skinsAhri', 'Wukong']);
    fireEvent.click(screen.getByRole('button', { name: 'Todos' }));

    fireEvent.click(screen.getByRole('button', { name: 'Lutador' }));
    expect(within(grid).getAllByRole('button')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'Selva' }));
    expect(within(grid).getAllByRole('button')).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Selva' }));
    fireEvent.click(screen.getByRole('button', { name: 'Lutador' }));

    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar campeão' }), { target: { value: 'monkey' } });
    expect(within(grid).getByRole('button')).toHaveAccessibleName(/^Wukong/);
    fireEvent.change(screen.getByRole('searchbox', { name: 'Buscar campeão' }), { target: { value: 'zzz' } });
    expect(screen.getByText('Nenhum campeão encontrado.')).toBeInTheDocument();
  });

  it('abre um campeão, pede as skins e mostra skins e cromas', () => {
    const { send, rerender } = setup({ collection });
    fireEvent.click(screen.getByRole('button', { name: /^Aatrox, na conta/ }));
    expect(send).toHaveBeenLastCalledWith({ type: 'openChampionSkins', championId: 266 });
    expect(screen.getByRole('heading', { name: 'Aatrox' })).toBeInTheDocument();
    expect(screen.getByText('Carregando as skins…')).toBeInTheDocument();

    // Skins de outro campeão (pedido antigo) não aparecem.
    rerender({ collection, championSkins: { championId: 103, skins: [] } });
    expect(screen.getByText('Carregando as skins…')).toBeInTheDocument();

    rerender({ collection, championSkins: aatroxSkins });
    expect(screen.getByText('1 de 2 skins · 1 de 2 cromas')).toBeInTheDocument();
    expect(screen.getByText('Desde 12 de mar. de 2021')).toBeInTheDocument();
    const skins = screen.getByRole('list', { name: 'Skins do campeão' });
    expect(within(skins).getAllByRole('button')).toHaveLength(3);
    const blood = within(skins).getByRole('button', { name: 'Aatrox Lua de Sangue, Lendária, não está na conta' });
    expect(blood.querySelector('img')?.getAttribute('src')).toMatch(/\/assets\/skin\/266002$/);
    expect(screen.queryByRole('list', { name: /^Cromas/ })).toBeNull();

    fireEvent.click(blood);
    expect(blood).toHaveAttribute('aria-pressed', 'true');
    const chromas = screen.getByRole('list', { name: 'Cromas de Aatrox Lua de Sangue' });
    expect(within(chromas).getAllByRole('listitem').map(li => li.textContent)).toEqual([
      'Rubi, na conta', 'Safira, não está na conta',
    ]);

    fireEvent.click(screen.getByRole('button', { name: 'Voltar para a Coleção' }));
    expect(screen.getByRole('list', { name: 'Grade de campeões' })).toBeInTheDocument();
  });

  it('com erro ao carregar, oferece tentar de novo', async () => {
    const send = vi.fn(() => Promise.resolve({ status: 'error' as const, code: 'lcu' as const, message: 'falhou' }));
    render(<CollectionPage connection={{ status: 'connected', error: null, state: makeState({ summoner }) }} send={send} />);
    expect(await screen.findByText('Não foi possível carregar a coleção: falhou')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Tentar de novo' }));
    expect(send).toHaveBeenCalledTimes(2);
  });
});

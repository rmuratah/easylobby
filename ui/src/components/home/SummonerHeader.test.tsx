import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { RankedEntry } from '../../bindings/RankedEntry';
import type { Summoner } from '../../bindings/Summoner';
import { SummonerHeader } from './SummonerHeader';

const summoner: Summoner = {
  gameName: 'haxixezinho', tagLine: 'anna', displayName: '', summonerLevel: 198, profileIconId: 1, puuid: 'p', summonerId: 1,
};
const entry = (overrides: Partial<RankedEntry> = {}): RankedEntry => ({
  tier: null, division: null, leaguePoints: 0, wins: 0, losses: 0, provisional: false, ...overrides,
});
const tile = (title: string) => screen.getByText(title).parentElement as HTMLElement;

describe('SummonerHeader', () => {
  it('mostra o emblema do elo de cada fila (#659)', () => {
    render(<SummonerHeader summoner={summoner} ranked={{
      soloDuo: entry({ tier: 'GRANDMASTER', leaguePoints: 1609 }),
      flex: entry({ tier: 'GOLD', division: 'I', leaguePoints: 37 }),
    }} />);
    expect(tile('Solo/Duo')).toHaveTextContent('Grão-Mestre · 1609 PDL');
    expect(tile('Solo/Duo').querySelector('img')).toHaveAttribute('src', expect.stringMatching(/rankedEmblem\/9$/));
    expect(tile('Flex').querySelector('img')).toHaveAttribute('src', expect.stringMatching(/rankedEmblem\/4$/));
  });

  it('sem classificação, um círculo neutro no lugar do emblema', () => {
    render(<SummonerHeader summoner={summoner} ranked={{ soloDuo: entry(), flex: entry() }} />);
    expect(tile('Solo/Duo')).toHaveTextContent('Sem classificação');
    expect(tile('Solo/Duo').querySelector('img')).toBeNull();
    expect(within(tile('Solo/Duo')).getByTestId('tier-emblem')).toHaveTextContent('—');
  });

  it('enquanto os ranks não chegaram, Indisponível e o círculo neutro', () => {
    render(<SummonerHeader summoner={summoner} ranked={null} />);
    expect(tile('Flex')).toHaveTextContent('Indisponível');
    expect(within(tile('Flex')).getByTestId('tier-emblem')).toHaveTextContent('—');
  });
});

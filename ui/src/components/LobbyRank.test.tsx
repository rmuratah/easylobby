import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { RankedEntry } from '../bindings/RankedEntry';
import type { RankedStats } from '../bindings/RankedStats';
import { LobbyRank } from './LobbyRank';

const entry = (overrides: Partial<RankedEntry> = {}): RankedEntry => ({
  tier: null, division: null, leaguePoints: 0, wins: 0, losses: 0, provisional: false, ...overrides,
});
const ranked: RankedStats = {
  soloDuo: entry({ tier: 'GOLD', division: 'II', leaguePoints: 45 }),
  flex: entry({ tier: 'MASTER', leaguePoints: 120 }),
};

describe('LobbyRank', () => {
  it('mostra o elo da Solo/Duo fora da Flex', () => {
    render(<LobbyRank ranked={ranked} queueId={420} />);
    expect(screen.getByTitle('Elo na Solo/Duo')).toHaveTextContent('Solo/Duo: Ouro II · 45 PDL');
    expect(screen.getByTitle('Elo na Solo/Duo').querySelector('img')).toHaveAttribute('src', expect.stringContaining('rankedEmblem/4'));
  });

  it('mostra o elo da Flex no lobby da Flex', () => {
    render(<LobbyRank ranked={ranked} queueId={440} />);
    expect(screen.getByTitle('Elo na Flex')).toHaveTextContent('Mestre · 120 PDL');
  });

  it('sem classificação, sem emblema nem PDL', () => {
    render(<LobbyRank ranked={{ ...ranked, soloDuo: entry() }} queueId={450} />);
    const chip = screen.getByTitle('Elo na Solo/Duo');
    expect(chip).toHaveTextContent('Sem classificação');
    expect(chip).not.toHaveTextContent('PDL');
    expect(chip.querySelector('img')).toBeNull();
  });

  it('some enquanto os ranks não chegaram', () => {
    const { container } = render(<LobbyRank ranked={null} queueId={420} />);
    expect(container).toBeEmptyDOMElement();
  });
});

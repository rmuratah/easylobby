import type { LiveGame } from '../bindings/LiveGame';
import type { LivePlayer } from '../bindings/LivePlayer';
import type { LivePlayerStats } from '../bindings/LivePlayerStats';

export function liveStats(overrides: Partial<LivePlayerStats> = {}): LivePlayerStats {
  return {
    rank: { tier: 'GOLD', division: 'II', leaguePoints: 45, wins: 30, losses: 20, provisional: false },
    previousSeason: { tier: 'SILVER', division: 'I' },
    recentGames: 20, championGames: 4, championWins: 3, mainPosition: 'middle', streak: 0,
    ...overrides,
  };
}

export function livePlayer(overrides: Partial<LivePlayer> = {}): LivePlayer {
  return {
    puuid: 'p1', me: false, bot: false, hidden: false, championId: 103, spell1Id: 4, spell2Id: 14,
    position: 'middle', gameName: 'Ana', tagLine: 'BR1', runes: null, stats: liveStats(),
    ...overrides,
  };
}

/** Uma Ranqueada Solo/Duo com um jogador de cada lado, mais um anônimo e um ainda carregando. */
export const testLiveGame: LiveGame = {
  gameId: 1, queueId: 420, queueName: 'Ranqueada Solo/Duo', mapId: 11, mapName: "Summoner's Rift", gameMode: 'CLASSIC',
  rankedQueue: 'soloDuo', gameTimeSecs: 125,
  teams: [
    {
      teamId: 100, averageTier: { tier: 'GOLD', division: 'II' },
      players: [
        livePlayer({ puuid: 'local', me: true, gameName: 'Victor', runes: { keystoneId: 8010, primaryStyleId: 8000, subStyleId: 8400 } }),
        livePlayer({ puuid: 'p2', gameName: '', tagLine: '', stats: null, championId: 266, position: 'top' }),
      ],
    },
    {
      teamId: 200, averageTier: null,
      players: [
        livePlayer({ puuid: 'p3', gameName: 'Rival', tagLine: 'NA1', stats: liveStats({ rank: null, streak: -4, previousSeason: null }) }),
        livePlayer({ puuid: '', hidden: true, gameName: '', tagLine: '', stats: null, championId: 26 }),
      ],
    },
  ],
};

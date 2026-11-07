import type { MatchSummary } from '../bindings/MatchSummary';

/** Uma partida do histórico para os testes, com o jogador logado vencendo de Ahri. */
export function match(gameId: number, overrides: Partial<MatchSummary> = {}, player: Partial<MatchSummary['player']> = {}): MatchSummary {
  return {
    gameId, queueId: 420, gameMode: 'CLASSIC', mapId: 11, createdAt: 1_000_000, durationSecs: 1805, result: 'win',
    ...overrides,
    player: {
      puuid: 'fake-puuid-0001', gameName: 'Jogador', tagLine: 'BR1', profileIconId: 29, teamId: 100, win: true,
      championId: 103, championLevel: 16, spell1Id: 4, spell2Id: 14, kills: 6, deaths: 2, assists: 8, cs: 180,
      gold: 12000, damageToChampions: 21000, visionScore: 22, items: [3071, 0, 0, 0, 0, 0, 3340],
      perkId: 8112, perkStyleId: 8100, perkSubStyleId: 8300, ...player,
    },
  };
}

import { describe, expect, it } from 'vitest';
import { parseMessage } from './protocol';
import { makeState } from '../test/state';
import { testGameResult } from '../test/gameResult';

const player = {
  puuid: 'fake-puuid-0001', gameName: 'fake-name-0001', tagLine: 'fake-tag-0001', profileIconId: 11, teamId: 200,
  win: true, championId: 432, championLevel: 13, spell1Id: 4, spell2Id: 14, kills: 1, deaths: 2, assists: 20, cs: 30,
  gold: 9000, damageToChampions: 8000, visionScore: 70, items: [3009, 0, 0, 0, 0, 0, 3364], perkId: 8465,
  perkStyleId: 8400, perkSubStyleId: 8300,
};
const advanced = testGameResult.teams[0]!.advanced[0]!;
const entry = { tier: 'MASTER', division: null, leaguePoints: 804, wins: 54, losses: 28, provisional: false };
const gameResult = {
  gameId: 1124, queueId: 420, queueName: 'Ranqueada Solo/Duo', gameMode: 'CLASSIC', ranked: true, durationSecs: 1551,
  result: 'win', localPuuid: 'fake-puuid-0001',
  teams: [{ teamId: 200, playerTeam: true, win: true, players: [player], advanced: [advanced] }],
  rankedChange: { queue: 'soloDuo', before: entry, after: { ...entry, leaguePoints: 868 }, leaguePointsDelta: 64 },
};
const parse = (value: unknown) => parseMessage(JSON.stringify({ type: 'gameResult', gameResult: value }));

describe('gameResult (#179)', () => {
  it('aceita o resultado, sem PDL e nulo', () => {
    expect(parse(gameResult)).toEqual({ type: 'gameResult', gameResult });
    expect(parse({ ...gameResult, rankedChange: null })).not.toBeNull();
    expect(parse({ ...gameResult, rankedChange: { ...gameResult.rankedChange, leaguePointsDelta: -18 } })).not.toBeNull();
    expect(parse(null)).toEqual({ type: 'gameResult', gameResult: null });
  });

  it('recusa um resultado malformado, no evento e no hello', () => {
    expect(() => parse({ ...gameResult, result: 'empate' })).toThrow();
    expect(() => parse({ ...gameResult, teams: [{ teamId: 200, playerTeam: true, players: [player] }] })).toThrow();
    expect(() => parse({ ...gameResult, rankedChange: { ...gameResult.rankedChange, queue: 'aram' } })).toThrow();
    expect(() => parse({ ...gameResult, queueName: undefined })).toThrow();
    // Detalhes avançados (#586).
    const team = gameResult.teams[0]!;
    expect(() => parse({ ...gameResult, teams: [{ ...team, advanced: [{ ...advanced, goldSpent: -1 }] }] })).toThrow();
    expect(() => parse({ ...gameResult, teams: [{ ...team, advanced: [{ ...advanced, perks: [{ id: 8112, vars: [1] }] }] }] })).toThrow();
    expect(() => parseMessage(JSON.stringify({ type: 'hello', protocol: 1, agentVersion: '0.1.0',
      state: makeState({ gameResult: { ...gameResult, durationSecs: -1 } as never }) }))).toThrow();
  });
});

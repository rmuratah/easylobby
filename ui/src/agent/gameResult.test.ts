import { describe, expect, it } from 'vitest';
import type { GameResult } from '../bindings/GameResult';
import type { RankedEntry } from '../bindings/RankedEntry';
import { compactNumber, localPlayer, perMinute, rankedChangeText, resultQueue } from './gameResult';
import { testGameResult } from '../test/gameResult';

const entry = (tier: string | null, division: string | null, leaguePoints: number): RankedEntry =>
  ({ tier, division, leaguePoints, wins: 10, losses: 10, provisional: false });

describe('resultQueue', () => {
  it('usa o nome que a LCU mandou', () => {
    expect(resultQueue(testGameResult, [])).toBe('Ranqueada Solo/Duo');
  });
  it('sem nome, usa a fila conhecida pelo id', () => {
    expect(resultQueue({ ...testGameResult, queueName: '', queueId: 450 }, [])).toBe('ARAM');
  });
  it('sem fila, usa o modo de jogo', () => {
    const result: GameResult = { ...testGameResult, queueName: '', queueId: 0, gameMode: 'CHERRY' };
    expect(resultQueue(result, [])).toBe('Arena');
    expect(resultQueue({ ...result, gameMode: '' }, [])).toBe('Partida');
  });
});

describe('localPlayer', () => {
  it('acha o jogador logado em qualquer time', () => {
    expect(localPlayer(testGameResult)?.championId).toBe(432);
  });
  it('sem puuid, ninguém', () => {
    expect(localPlayer({ ...testGameResult, localPuuid: '' })).toBeNull();
  });
});

describe('rankedChangeText', () => {
  it('PDL ganhos e perdidos', () => {
    const change = { queue: 'soloDuo' as const, before: entry('MASTER', null, 804), after: entry('MASTER', null, 868), leaguePointsDelta: 64 };
    expect(rankedChangeText(change, true)).toBe('+64 PDL');
    expect(rankedChangeText({ ...change, leaguePointsDelta: -18 }, false)).toBe('-18 PDL');
  });
  it('promoção, rebaixamento e primeira classificação', () => {
    const change = { queue: 'soloDuo' as const, before: entry('GOLD', 'II', 90), after: entry('GOLD', 'I', 10), leaguePointsDelta: null };
    expect(rankedChangeText(change, true)).toBe('Promovido a Ouro I');
    expect(rankedChangeText({ ...change, after: entry('SILVER', 'I', 75) }, false)).toBe('Rebaixado para Prata I');
    expect(rankedChangeText({ ...change, before: entry(null, null, 0) }, true)).toBe('Classificado em Ouro I');
  });
});

it('formata números do placar', () => {
  expect(perMinute(121, 1551)).toBe('4,7');
  expect(perMinute(5, 0)).toBe('0');
  expect(compactNumber(950)).toBe('950');
  expect(compactNumber(9127)).toBe('9,1 mil');
});

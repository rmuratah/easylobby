import type { GameResult } from '../bindings/GameResult';

/** O resultado da ranqueada gravada em `16.22-pos-jogo` (#177), como o agente manda (#179). */
export const testGameResult: GameResult = {
  gameId: 1124,
  queueId: 420,
  queueName: 'Ranqueada Solo/Duo',
  gameMode: 'CLASSIC',
  ranked: true,
  durationSecs: 1551,
  result: 'win',
  localPuuid: 'fake-puuid-0001',
  teams: [
    { teamId: 200, playerTeam: true, win: true, players: [
      {"puuid": "fake-puuid-0092", "gameName": "fake-name-0002", "tagLine": "fake-tag-0001", "profileIconId": 7123, "teamId": 200, "win": true, "championId": 897, "championLevel": 15, "spell1Id": 4, "spell2Id": 12, "kills": 3, "deaths": 4, "assists": 8, "cs": 121, "gold": 9127, "damageToChampions": 14268, "visionScore": 16, "items": [6665, 6662, 3111, 1120, 2022, 0, 3340], "perkId": 8437, "perkStyleId": 8400, "perkSubStyleId": 8000},
      {"puuid": "fake-puuid-0093", "gameName": "fake-name-0130", "tagLine": "fake-tag-0002", "profileIconId": 5915, "teamId": 200, "win": true, "championId": 234, "championLevel": 16, "spell1Id": 4, "spell2Id": 11, "kills": 11, "deaths": 2, "assists": 9, "cs": 205, "gold": 13809, "damageToChampions": 17054, "visionScore": 25, "items": [3111, 6676, 6673, 3036, 0, 0, 3340], "perkId": 9923, "perkStyleId": 8100, "perkSubStyleId": 8000},
      {"puuid": "fake-puuid-0095", "gameName": "fake-name-0003", "tagLine": "fake-tag-0003", "profileIconId": 3150, "teamId": 200, "win": true, "championId": 55, "championLevel": 16, "spell1Id": 14, "spell2Id": 4, "kills": 21, "deaths": 3, "assists": 5, "cs": 203, "gold": 16079, "damageToChampions": 36963, "visionScore": 17, "items": [3157, 1058, 3041, 3175, 3100, 4645, 3364], "perkId": 8010, "perkStyleId": 8000, "perkSubStyleId": 8100},
      {"puuid": "fake-puuid-0094", "gameName": "fake-name-0132", "tagLine": "fake-tag-0002", "profileIconId": 4096, "teamId": 200, "win": true, "championId": 101, "championLevel": 15, "spell1Id": 4, "spell2Id": 7, "kills": 5, "deaths": 3, "assists": 17, "cs": 189, "gold": 12193, "damageToChampions": 29229, "visionScore": 21, "items": [6655, 4645, 3041, 1056, 0, 0, 3340], "perkId": 8229, "perkStyleId": 8200, "perkSubStyleId": 8000},
      {"puuid": "fake-puuid-0001", "gameName": "fake-name-0001", "tagLine": "fake-tag-0001", "profileIconId": 11, "teamId": 200, "win": true, "championId": 432, "championLevel": 13, "spell1Id": 4, "spell2Id": 14, "kills": 2, "deaths": 4, "assists": 24, "cs": 18, "gold": 8933, "damageToChampions": 11716, "visionScore": 80, "items": [3009, 3107, 3877, 2524, 0, 3222, 3364], "perkId": 8112, "perkStyleId": 8100, "perkSubStyleId": 8300},
    ] },
    { teamId: 100, playerTeam: false, win: false, players: [
      {"puuid": "fake-puuid-0091", "gameName": "fake-name-0004", "tagLine": "fake-tag-0004", "profileIconId": 6795, "teamId": 100, "win": false, "championId": 41, "championLevel": 14, "spell1Id": 4, "spell2Id": 12, "kills": 2, "deaths": 9, "assists": 3, "cs": 162, "gold": 10542, "damageToChampions": 16792, "visionScore": 20, "items": [1055, 1083, 3508, 3111, 3036, 3155, 3363], "perkId": 8992, "perkStyleId": 8200, "perkSubStyleId": 8000},
      {"puuid": "fake-puuid-0090", "gameName": "fake-name-0005", "tagLine": "fake-tag-0002", "profileIconId": 6623, "teamId": 100, "win": false, "championId": 876, "championLevel": 15, "spell1Id": 4, "spell2Id": 11, "kills": 7, "deaths": 5, "assists": 2, "cs": 211, "gold": 12918, "damageToChampions": 17811, "visionScore": 13, "items": [4633, 6653, 2420, 3158, 1082, 3116, 3364], "perkId": 8010, "perkStyleId": 8000, "perkSubStyleId": 8300},
      {"puuid": "fake-puuid-0087", "gameName": "fake-name-0135", "tagLine": "fake-tag-0005", "profileIconId": 6097, "teamId": 100, "win": false, "championId": 147, "championLevel": 13, "spell1Id": 12, "spell2Id": 4, "kills": 1, "deaths": 6, "assists": 5, "cs": 159, "gold": 8510, "damageToChampions": 16313, "visionScore": 18, "items": [2503, 3175, 6653, 1056, 1057, 0, 3363], "perkId": 8369, "perkStyleId": 8300, "perkSubStyleId": 8100},
      {"puuid": "fake-puuid-0089", "gameName": "fake-name-0136", "tagLine": "fake-tag-0006", "profileIconId": 4764, "teamId": 100, "win": false, "championId": 145, "championLevel": 14, "spell1Id": 21, "spell2Id": 4, "kills": 6, "deaths": 10, "assists": 2, "cs": 207, "gold": 13128, "damageToChampions": 13808, "visionScore": 15, "items": [6672, 3124, 1026, 3302, 1052, 0, 3363], "perkId": 8008, "perkStyleId": 8000, "perkSubStyleId": 8300},
      {"puuid": "fake-puuid-0088", "gameName": "fake-name-0137", "tagLine": "fake-tag-0007", "profileIconId": 6382, "teamId": 100, "win": false, "championId": 12, "championLevel": 10, "spell1Id": 4, "spell2Id": 7, "kills": 0, "deaths": 12, "assists": 7, "cs": 30, "gold": 6527, "damageToChampions": 3939, "visionScore": 53, "items": [3869, 3109, 2031, 3158, 3067, 1057, 3364], "perkId": 8439, "perkStyleId": 8400, "perkSubStyleId": 8300},
    ] },
  ],
  rankedChange: {
    queue: 'soloDuo',
    before: { tier: 'MASTER', division: null, leaguePoints: 804, wins: 54, losses: 28, provisional: false },
    after: { tier: 'MASTER', division: null, leaguePoints: 868, wins: 55, losses: 28, provisional: false },
    leaguePointsDelta: 64,
  },
};

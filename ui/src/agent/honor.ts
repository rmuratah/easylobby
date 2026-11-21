import type { HonorBallot } from '../bindings/HonorBallot';
import type { GameResult } from '../bindings/GameResult';
import type { HonorCandidate } from '../bindings/HonorCandidate';
import type { MatchPlayer } from '../bindings/MatchPlayer';

// Contas da cédula de honra do fim de partida (#182).

/** Honras que ainda dá para dar na cédula. */
export function votesLeft(ballot: HonorBallot): number {
  return Math.max(0, ballot.votes - ballot.honoredPuuids.length);
}

/** Jogadores da cédula que o jogador já honrou, aliados primeiro. */
export function honoredPlayers(ballot: HonorBallot): HonorCandidate[] {
  return [...ballot.eligibleAllies, ...ballot.eligibleOpponents].filter(p => ballot.honoredPuuids.includes(p.puuid));
}

/** Nome do jogador na cédula, ou o campeão quando é bot. */
export function playerName(player: HonorCandidate): string {
  return player.bot ? `${player.championName} (bot)` : player.summonerName || player.championName;
}

/**
 * O jogador da cédula no resultado da partida (#629), pelo `puuid`; se não
 * achar (o resultado vem sem `puuid` para perfil privado), pelo campeão, que
 * não se repete numa partida. `null` sem resultado ou se for de outra partida.
 */
export function ballotPlayerStats(result: GameResult | null, ballot: HonorBallot, player: HonorCandidate): MatchPlayer | null {
  if (!result || result.gameId !== ballot.gameId) return null;
  const players = result.teams.flatMap(team => team.players);
  return players.find(p => p.puuid !== '' && p.puuid === player.puuid)
    ?? players.find(p => p.championId === player.championId)
    ?? null;
}

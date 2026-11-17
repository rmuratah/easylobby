import type { HonorBallot } from '../bindings/HonorBallot';
import type { HonorCandidate } from '../bindings/HonorCandidate';

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

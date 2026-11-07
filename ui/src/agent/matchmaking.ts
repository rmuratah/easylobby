import type { Matchmaking } from '../bindings/Matchmaking';

/**
 * Se a busca está parada por uma punição (dodge, abandono ou baixa prioridade), com o tempo correndo.
 * Nesse estado o jogador pode sair do lobby, como no client oficial.
 */
export function queuePenalty(matchmaking: Matchmaking | null | undefined): boolean {
  if (!matchmaking || matchmaking.searchState === 'Found') return false;
  return matchmaking.searchState === 'Error' || matchmaking.searchState === 'AbandonedLowPriorityQueue'
    || matchmaking.errors.length > 0 || matchmaking.lowPriorityTimeRemaining > 0;
}

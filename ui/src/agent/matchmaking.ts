import type { Matchmaking } from '../bindings/Matchmaking';
import type { MatchmakingError } from '../bindings/MatchmakingError';

/**
 * Se a busca está parada por uma punição (dodge, abandono ou baixa prioridade), com o tempo correndo.
 * Nesse estado o jogador pode sair do lobby, como no client oficial.
 */
export function queuePenalty(matchmaking: Matchmaking | null | undefined): boolean {
  if (!matchmaking || matchmaking.searchState === 'Found') return false;
  return matchmaking.searchState === 'Error' || matchmaking.searchState === 'AbandonedLowPriorityQueue'
    || matchmaking.errors.length > 0 || matchmaking.lowPriorityTimeRemaining > 0;
}

/** Segundos como `m:ss`. */
export function duration(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  return `${minutes}:${String(total % 60).padStart(2, '0')}`;
}

/** Por que a busca parou, para cada `errorType` conhecido da LCU (#502). */
const ERROR_CAUSES: Record<string, string> = {
  QUEUE_DODGER: 'Você saiu de uma seleção de campeões',
  LEAVER_BUSTED: 'Você abandonou ou ficou ausente em partidas recentes',
  LEAVER_BUSTER_TAINTED_WARNING: 'Você abandonou ou ficou ausente em partidas recentes',
  LEAVER_BUSTER_QUEUE_LOCKOUT: 'Você abandonou ou ficou ausente em partidas recentes',
  LOW_PRIORITY_QUEUE: 'Você está na fila de baixa prioridade por abandonar partidas',
  PLAYER_TIMED_RESTRICTION: 'Sua conta está com uma restrição temporária',
  RANKED_RESTRICTED: 'Sua conta está com uma restrição nas ranqueadas',
  RANKED_SUSPENDED: 'Sua conta está suspensa das ranqueadas',
  RANKED_MIN_LEVEL: 'Sua conta ainda não tem o nível necessário para as ranqueadas',
  RANKED_NUM_CHAMPS: 'Você ainda não tem campeões suficientes para as ranqueadas',
  QUEUE_UNAVAILABLE: 'Esta fila não está disponível agora',
  SERVICE_ERROR: 'O servidor de partidas da Riot teve um problema',
};

/**
 * O texto de um erro da busca (#502), em vez do código cru da LCU: "Você saiu
 * de uma seleção de campeões e precisa esperar 0:42 para entrar na fila." Para
 * um código sem texto, usa a mensagem da Riot, se vier, ou uma frase genérica
 * com o código entre parênteses.
 */
export function matchmakingErrorText(error: MatchmakingError): string {
  const wait = error.penaltyTimeRemaining > 0 ? duration(error.penaltyTimeRemaining) : null;
  const cause = ERROR_CAUSES[error.errorType.trim().toUpperCase()];
  if (cause) {
    return wait ? `${cause} e precisa esperar ${wait} para entrar na fila.` : `${cause}.`;
  }
  const riot = error.message.trim();
  const base = riot || (error.errorType.trim()
    ? `O League of Legends não deixou entrar na fila agora (${error.errorType.trim()})`
    : 'O League of Legends não deixou entrar na fila agora');
  const sentence = /[.!?]$/.test(base) ? base : `${base}.`;
  return wait ? `${sentence} Aguarde ${wait} para tentar de novo.` : sentence;
}

/** O aviso da fila de baixa prioridade (#502). */
export function lowPriorityText(seconds: number): string {
  return `Você abandonou partidas recentes e vai esperar ${duration(seconds)} na fila de baixa prioridade antes da busca começar.`;
}

import type { Lobby } from '../bindings/Lobby';

/** Texto para um motivo que o agente mandou sem tradução (código novo da LCU). */
export const GENERIC_RESTRICTION = 'O League of Legends não deixa iniciar a busca com este grupo agora.';

/**
 * Por que a fila não pode começar (#227), no texto que o agente mandou, sem
 * repetir. Vazia com a fila liberada: a LCU às vezes manda motivos mesmo
 * com `canStartActivity`, e aí eles não valem.
 */
export function restrictionReasons(lobby: Lobby | null): string[] {
  if (!lobby || lobby.canStartActivity) return [];
  return [...new Set(lobby.restrictions.map(r => r.message ?? GENERIC_RESTRICTION))];
}

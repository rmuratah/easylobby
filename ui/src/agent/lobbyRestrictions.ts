import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import { draftComplete, draftFromSlots } from './quickPlay';

/** Texto para um motivo que o agente mandou sem tradução (código novo da LCU). */
export const GENERIC_RESTRICTION = 'O League of Legends não deixa iniciar a busca com este grupo agora.';

/**
 * Por que a fila não pode começar (#227), no texto que o agente mandou, sem
 * repetir. Vazia com a fila liberada: a LCU às vezes manda motivos mesmo
 * com `canStartActivity`, e aí eles não valem.
 */
export function restrictionReasons(lobby: Lobby | null): string[] {
  if (!lobby || lobby.canStartActivity) return [];
  // O código vai junto no genérico, para dar para traduzir depois.
  return [...new Set(lobby.restrictions.map(r => r.message ?? `${GENERIC_RESTRICTION} (${r.code})`))];
}

/**
 * Escolhas obrigatórias que faltam antes da busca (#334): posições primária e
 * secundária (Alternada e ranqueadas) e as duas posições com campeão (Jogo
 * Dinâmico), de cada membro. Vazia quando não falta nada. A ordem das 5
 * posições do Rift Clássico (#230) ainda não tem tela e fica de fora.
 */
export function pendingChoices(lobby: Lobby | null, me: string | undefined, nameOf: (member: LobbyMember) => string): string[] {
  if (!lobby || lobby.custom) return [];
  const missing = lobby.members.filter(member => {
    if (lobby.showQuickPlaySlots) return !draftComplete(draftFromSlots(member.playerSlots));
    if (lobby.showPositionSelector && !lobby.orderedPositions) {
      const { firstPosition: first, secondPosition: second } = member;
      return first === 'unselected' || (first !== 'fill' && second === 'unselected');
    }
    return false;
  });
  const what = lobby.showQuickPlaySlots ? 'as duas posições e um campeão para cada uma' : 'as posições primária e secundária';
  return missing.map(member => member.puuid === me ? `Escolha ${what}.` : `${nameOf(member)} ainda não escolheu ${what}.`);
}

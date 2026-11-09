import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { GameflowPhase } from '../bindings/GameflowPhase';
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

/**
 * Membros que ainda não voltaram ao lobby (#346): depois da partida, quem está
 * na tela de fim de jogo continua no grupo, mas a busca só libera quando todos
 * voltam. Só na fase `Lobby`, porque na fila e na seleção a LCU marca todos
 * como não prontos. A personalizada fica de fora.
 */
export function awayMembers(lobby: Lobby | null, phase: GameflowPhase | null): LobbyMember[] {
  if (!lobby || lobby.custom || phase !== 'Lobby') return [];
  return lobby.members.filter(member => !member.ready);
}

/** O aviso para cada membro de {@link awayMembers}. */
export function awayReasons(away: LobbyMember[], me: string | undefined, nameOf: (member: LobbyMember) => string): string[] {
  return away.map(member => member.puuid === me ? 'Você ainda não voltou ao lobby.' : `${nameOf(member)} ainda não voltou ao lobby.`);
}

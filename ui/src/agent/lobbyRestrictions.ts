import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { LobbyRestriction } from '../bindings/LobbyRestriction';
import type { GameflowPhase } from '../bindings/GameflowPhase';
import { draftComplete, draftFromSlots } from './quickPlay';

/** Texto para um motivo que o agente mandou sem tradução (código novo da LCU). */
export const GENERIC_RESTRICTION = 'O League of Legends não deixa iniciar a busca com este grupo agora.';

/** Nomes em lista: "A", "A e B", "A, B e C". */
function joinNames(names: string[]): string {
  return names.length < 2 ? names.join('') : `${names.slice(0, -1).join(', ')} e ${names.at(-1)}`;
}

/** Membros do lobby citados no motivo, na ordem da LCU (#411). Quem não está no grupo fica de fora. */
function blockedBy(lobby: Lobby, restriction: LobbyRestriction): LobbyMember[] {
  return restriction.puuids.flatMap(puuid => lobby.members.find(member => member.puuid === puuid) ?? []);
}

/**
 * O texto de um motivo (#227), com quem está bloqueando a fila quando a LCU
 * diz (#411): "Algum jogador está com punição de fila." vira "Ana está com
 * punição de fila." (ou "Você está…"); com mais de um jogador, ou num motivo
 * que não começa por "Algum jogador", os nomes vão no fim, entre parênteses.
 */
function reasonText(lobby: Lobby, restriction: LobbyRestriction, me: string | undefined, nameOf: (member: LobbyMember) => string): string {
  const members = blockedBy(lobby, restriction);
  const names = members.map(member => member.puuid === me ? 'você' : nameOf(member));
  if (restriction.message === null) {
    if (!names.length) return `${GENERIC_RESTRICTION} (${restriction.code})`;
    const who = joinNames(names);
    const verb = names.length > 1 ? 'não podem' : 'não pode';
    return `${who.charAt(0).toUpperCase()}${who.slice(1)} ${verb} entrar nesta fila agora. (${restriction.code})`;
  }
  if (!names.length) return restriction.message;
  const subject = 'Algum jogador ';
  if (names.length === 1 && restriction.message.startsWith(subject)) {
    const who = members[0].puuid === me ? 'Você' : names[0];
    return `${who} ${restriction.message.slice(subject.length)}`;
  }
  return `${restriction.message.replace(/\.$/, '')} (${joinNames(names)}).`;
}

/**
 * Por que a fila não pode começar (#227), no texto que o agente mandou, sem
 * repetir, dizendo quem está bloqueando quando a LCU diz (#411). Vazia com a
 * fila liberada: a LCU às vezes manda motivos mesmo com `canStartActivity`, e
 * aí eles não valem.
 */
export function restrictionReasons(lobby: Lobby | null, me: string | undefined, nameOf: (member: LobbyMember) => string): string[] {
  if (!lobby || lobby.canStartActivity) return [];
  return [...new Set(lobby.restrictions.map(r => reasonText(lobby, r, me, nameOf)))];
}

/**
 * Motivos de cada membro que está bloqueando a fila (#411), pelo `puuid`,
 * para marcar o card dele. Vazio com a fila liberada.
 */
export function blockingMembers(lobby: Lobby | null): Map<string, string[]> {
  const blocked = new Map<string, string[]>();
  if (!lobby || lobby.canStartActivity) return blocked;
  for (const restriction of lobby.restrictions) {
    const text = restriction.message ?? `${GENERIC_RESTRICTION} (${restriction.code})`;
    for (const member of blockedBy(lobby, restriction)) {
      const list = blocked.get(member.puuid) ?? [];
      if (!list.includes(text)) list.push(text);
      blocked.set(member.puuid, list);
    }
  }
  return blocked;
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

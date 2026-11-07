import type { Friend } from '../bindings/Friend';
import type { InvitationState } from '../bindings/InvitationState';
import type { Lobby } from '../bindings/Lobby';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import { isOnline } from './friends';
import { queueNames } from './queues';

/** Convites que ainda ocupam uma vaga no grupo: o amigo pode aceitar a qualquer momento. */
const openStates = { Requested: true, Pending: true, OnHold: true } as Partial<Record<InvitationState, true>>;
export const isOpenInvitation = (state: InvitationState) => !!openStates[state];

/** Nome de quem convidou: a LCU pode mandar vazio com o Riot ID. */
export function inviterName(invitation: ReceivedInvitation): string {
  return invitation.fromSummonerName || 'Um amigo';
}

/** Nome da fila do convite, ou "outra fila" quando o app não conhece. */
export function invitationQueueName(invitation: ReceivedInvitation): string {
  return invitation.queue ? queueNames[invitation.queue] : `outra fila (ID ${invitation.queueId})`;
}

/** Vagas livres no grupo, descontando quem já está e quem tem convite em aberto. */
export function freeSlots(lobby: Lobby): number {
  const open = lobby.invitations.filter(i => isOpenInvitation(i.state)).length;
  return Math.max(0, lobby.maxLobbySize - lobby.members.length - open);
}

/**
 * Amigos que dá para convidar agora: online no PC (no celular não entram no
 * lobby), com `summonerId` (sem ele a LCU não entrega o convite), fora do
 * grupo e sem convite em aberto.
 */
export function invitableFriends(lobby: Lobby, friends: Friend[]): Friend[] {
  const taken = new Set([
    ...lobby.members.map(m => m.puuid),
    ...lobby.invitations.filter(i => isOpenInvitation(i.state)).map(i => i.puuid),
  ]);
  return friends.filter(f => isOnline(f) && f.availability !== 'mobile' && f.summonerId > 0 && !!f.puuid && !taken.has(f.puuid));
}

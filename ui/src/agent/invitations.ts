import type { Friend } from '../bindings/Friend';
import type { GameQueue } from '../bindings/GameQueue';
import type { InvitationState } from '../bindings/InvitationState';
import type { Lobby } from '../bindings/Lobby';
import type { PlayerCard } from '../bindings/PlayerCard';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import { findPlayerCard, friendDisplayName, friendNick, isOnline, playerCardName } from './friends';
import { queueNames } from './queues';

/** Convites que ainda ocupam uma vaga no grupo: o amigo pode aceitar a qualquer momento. */
const openStates = { Requested: true, Pending: true, OnHold: true } as Partial<Record<InvitationState, true>>;
export const isOpenInvitation = (state: InvitationState) => !!openStates[state];

/**
 * Amigo que mandou o convite, achado pelo `summonerId`. `undefined` quando
 * quem convidou não está na lista de amigos.
 */
function invitationSender(invitation: ReceivedInvitation, friends: Friend[]): Friend | undefined {
  if (invitation.fromSummonerId <= 0) return undefined;
  return friends.find(f => f.summonerId === invitation.fromSummonerId);
}

/**
 * Nome de quem convidou: o nick do amigo (sem a tag, como na lista de
 * amigos), o de quem não é amigo que o agente buscou (#675), o nome que a
 * LCU mandou (pode vir vazio com o Riot ID) ou "Um amigo".
 */
export function inviterName(invitation: ReceivedInvitation, friends: Friend[], cards: readonly PlayerCard[] = []): string {
  const sender = invitationSender(invitation, friends);
  const card = sender ? undefined : findPlayerCard(cards, '', invitation.fromSummonerId);
  return (sender && friendNick(sender)) || card?.gameName || invitation.fromSummonerName || 'Um amigo';
}

/**
 * Quem convidou, para o aviso: o nick, o Riot ID completo (para a dica) e o
 * ícone de invocador, do amigo ou de quem não é amigo (#675). Ícone `null`
 * enquanto o agente não buscou.
 */
export function inviter(invitation: ReceivedInvitation, friends: Friend[], cards: readonly PlayerCard[] = []):
  { name: string; riotId: string | undefined; profileIconId: number | null } {
  const name = inviterName(invitation, friends, cards);
  const sender = invitationSender(invitation, friends);
  if (sender) return { name, riotId: friendDisplayName(sender), profileIconId: sender.profileIconId };
  const card = findPlayerCard(cards, '', invitation.fromSummonerId);
  if (card) return { name, riotId: playerCardName(card), profileIconId: card.profileIconId || null };
  return { name, riotId: undefined, profileIconId: null };
}

/**
 * Nome da fila do convite: o nome fixo, o da lista de filas da LCU (ARAM:
 * Desordem e outros modos, #146), ou "outra fila" quando nenhum dos dois
 * conhece.
 */
export function invitationQueueName(invitation: ReceivedInvitation, queues: GameQueue[]): string {
  if (invitation.queue) return queueNames[invitation.queue];
  const listed = queues.find(q => q.queueId === invitation.queueId);
  return listed?.name || listed?.description || `outra fila (ID ${invitation.queueId})`;
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

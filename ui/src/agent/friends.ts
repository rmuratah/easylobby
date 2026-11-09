import type { Activity } from '../bindings/Activity';
import type { Availability } from '../bindings/Availability';
import type { Friend } from '../bindings/Friend';
import type { FriendGroup } from '../bindings/FriendGroup';
import type { LeagueStatus } from '../bindings/LeagueStatus';
import type { LobbyMember } from '../bindings/LobbyMember';

export const availabilityNames = {
  online: 'Online',
  busy: 'Ocupado',
  away: 'Ausente',
  mobile: 'No celular',
  offline: 'Offline',
  unknown: 'Desconhecido',
} satisfies Record<Availability, string>;

/** Cor da bolinha de status, no padrão do client oficial. */
export const availabilityColors = {
  online: 'bg-status-online',
  busy: 'bg-status-game',
  away: 'bg-status-away',
  mobile: 'bg-status-offline',
  offline: 'bg-control',
  unknown: 'bg-control',
} satisfies Record<Availability, string>;

const leagueStatusNames = {
  outOfGame: 'No client',
  inLobby: 'No lobby',
  inQueue: 'Na fila',
  championSelect: 'Seleção de campeões',
  inGame: 'Em partida',
  spectating: 'Assistindo',
  unknown: 'No LoL',
} satisfies Record<LeagueStatus, string>;

/**
 * Nomes das filas pelo `queueId`. Só as filas que o app conhece; as outras
 * aparecem sem nome.
 */
const queueIdNames: Record<number, string> = {
  400: 'Normal',
  420: 'Ranqueada Solo/Duo',
  440: 'Ranqueada Flex',
  450: 'ARAM',
  480: 'Jogo Dinâmico',
};

/** Disponibilidades que contam como online (o resto vai para o grupo offline). */
export function isOnline(friend: Friend): boolean {
  return friend.availability !== 'offline' && friend.availability !== 'unknown';
}

/** Riot ID (`nome#tag`), ou o nome antigo de invocador em contas sem Riot ID. */
export function friendDisplayName(friend: Friend): string {
  if (friend.gameName) return friend.tagLine ? `${friend.gameName}#${friend.tagLine}` : friend.gameName;
  return friend.name || 'Amigo';
}

/**
 * Nick do amigo, sem a tag (`Ana`, e não `Ana#BR1`), como a lista de amigos e
 * o chat mostram. A tag fica em {@link friendDisplayName}, para o tooltip.
 */
export function friendNick(friend: Friend): string {
  return friend.gameName || friend.name || 'Amigo';
}

/** Nome da pasta padrão na LCU. */
const DEFAULT_GROUP_NAME = '**Default';

/** Nome da pasta para mostrar: a padrão vira "Geral", como no client oficial. */
export function friendGroupName(name: string): string {
  if (!name || name === DEFAULT_GROUP_NAME) return 'Geral';
  return name;
}

/** Uma pasta de amigos pronta para desenhar. */
export type FriendFolder = {
  id: number;
  name: string;
  /** Se começa fechada (o jogador deixou fechada no client oficial). */
  collapsed: boolean;
  /** Amigos da pasta, na ordem do agente. */
  friends: Friend[];
};

/**
 * Separa os amigos nas pastas do client oficial, na ordem dele. Uma pasta que
 * aparece nos amigos mas não veio na lista de pastas (a LCU não mandou, por
 * exemplo) entra no fim, a padrão primeiro e as outras por nome. Pastas sem
 * nenhum dos amigos dados ficam de fora.
 */
export function groupFriends(friends: Friend[], groups: FriendGroup[]): FriendFolder[] {
  const folders = new Map<number, FriendFolder>();
  for (const group of groups) {
    folders.set(group.id, { id: group.id, name: friendGroupName(group.name), collapsed: group.collapsed, friends: [] });
  }
  const extra: FriendFolder[] = [];
  for (const friend of friends) {
    let folder = folders.get(friend.groupId);
    if (!folder) {
      folder = { id: friend.groupId, name: friendGroupName(friend.groupName), collapsed: false, friends: [] };
      folders.set(friend.groupId, folder);
      extra.push(folder);
    }
    folder.friends.push(friend);
  }
  extra.sort((a, b) => Number(b.id === 0) - Number(a.id === 0) || a.name.localeCompare(b.name, 'pt-BR'));
  const ordered = [...folders.values()].filter(folder => !extra.includes(folder)).concat(extra);
  return ordered.filter(folder => folder.friends.length > 0);
}

/** Minutos desde `since`, ou `null` sem horário ou com horário no futuro. */
function minutesSince(since: number | null, now: number): number | null {
  if (since === null || now < since) return null;
  return Math.floor((now - since) / 60_000);
}

/**
 * Texto do que o amigo está fazendo, como "Em partida · Ranqueada Solo/Duo · 12 min".
 * Sem atividade, mostra a disponibilidade.
 */
export function describeActivity(friend: Friend, now: number): string {
  const activity: Activity | null = friend.activity;
  if (!activity) return availabilityNames[friend.availability];
  if (activity.type === 'otherGame') return activity.productName ? `Jogando ${activity.productName}` : 'Em outro jogo da Riot';
  const parts = [leagueStatusNames[activity.status]];
  const queue = activity.queueId !== null ? queueIdNames[activity.queueId] : undefined;
  if (queue && activity.status !== 'outOfGame') parts.push(queue);
  const minutes = activity.status === 'inQueue' || activity.status === 'inGame' || activity.status === 'spectating'
    ? minutesSince(activity.since, now) : null;
  if (minutes !== null) parts.push(`${minutes} min`);
  return parts.join(' · ');
}

/**
 * Riot ID de outro jogador (no lobby ou convidado), achado na lista de amigos
 * pelo `puuid` ou, na falta dele, pelo `summonerId`: a LCU não manda o nome
 * de quem está no grupo. "Jogador" quando não é amigo; para um membro do
 * lobby, use {@link memberName}, que acha também quem não é amigo.
 */
export function playerName(puuid: string, summonerId: number, friends: Friend[]): string {
  const friend = (puuid ? friends.find(f => f.puuid === puuid) : undefined)
    ?? (summonerId > 0 ? friends.find(f => f.summonerId === summonerId) : undefined);
  return friend ? friendDisplayName(friend) : 'Jogador';
}

/**
 * Riot ID de um membro do lobby (#325): o que o agente buscou pelo `puuid`
 * ou, enquanto ele não chega, o da lista de amigos. "Jogador" sem nenhum.
 */
export function memberName(member: LobbyMember, friends: Friend[]): string {
  if (member.gameName) return member.tagLine ? `${member.gameName}#${member.tagLine}` : member.gameName;
  return playerName(member.puuid, member.summonerId, friends);
}

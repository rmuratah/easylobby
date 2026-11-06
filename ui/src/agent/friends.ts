import type { Activity } from '../bindings/Activity';
import type { Availability } from '../bindings/Availability';
import type { Friend } from '../bindings/Friend';
import type { LeagueStatus } from '../bindings/LeagueStatus';

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
  online: 'bg-emerald-400',
  busy: 'bg-rose-400',
  away: 'bg-amber-400',
  mobile: 'bg-slate-400',
  offline: 'bg-slate-600',
  unknown: 'bg-slate-600',
} satisfies Record<Availability, string>;

export const leagueStatusNames = {
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

/** Minutos desde `since`, ou `null` sem horário ou com horário no futuro. */
export function minutesSince(since: number | null, now: number): number | null {
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

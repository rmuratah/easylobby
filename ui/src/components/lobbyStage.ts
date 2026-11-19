import type { LobbyMember } from '../bindings/LobbyMember';

export type Seat = { kind: 'member'; member: LobbyMember } | { kind: 'free'; index: number };

/**
 * Ordem do palco (#600): o jogador no meio; os outros, e depois as vagas livres,
 * ocupam os lugares mais perto do centro primeiro, alternando esquerda e direita.
 */
export function stageSeats(me: LobbyMember | undefined, others: LobbyMember[], free: number): Seat[] {
  const rest: Seat[] = [
    ...others.map(member => ({ kind: 'member' as const, member })),
    ...Array.from({ length: free }, (_, index) => ({ kind: 'free' as const, index })),
  ];
  if (!me) return rest;
  const total = rest.length + 1;
  const center = Math.floor(total / 2);
  const seats: Seat[] = new Array(total);
  seats[center] = { kind: 'member', member: me };
  const around: number[] = [];
  for (let step = 1; around.length < rest.length; step++) {
    if (center - step >= 0) around.push(center - step);
    if (center + step < total) around.push(center + step);
  }
  around.forEach((place, index) => { seats[place] = rest[index]; });
  return seats;
}

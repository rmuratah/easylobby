import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { SubteamSlot } from '../bindings/SubteamSlot';

/** Uma vaga de uma equipe da Arena (#224): os índices como a LCU manda e quem está nela. */
export type ArenaSlot = { slot: SubteamSlot; member: LobbyMember | null };
export type ArenaTeam = { team: number; slots: ArenaSlot[] };
export type ArenaLayout = { teams: ArenaTeam[]; unplaced: LobbyMember[] };

/**
 * Base dos índices de equipe e de vaga. A gravação 16.22 não tem um lobby da
 * Arena: o mock começa em 1 e a LCU ainda precisa ser conferida no LoL real.
 * Vale 1, a não ser que algum jogador chegue com um índice 0.
 */
export function subteamBase(members: LobbyMember[]): 0 | 1 {
  return members.some(m => m.subteam && (m.subteam.team === 0 || m.subteam.position === 0)) ? 0 : 1;
}

/**
 * Monta as equipes do lobby da Arena (#224): `subteams.count` equipes com
 * `subteams.size` vagas cada, com quem está em cada vaga. Quem ainda não tem
 * vaga (ou chegou numa vaga fora do layout, ou repetida) vai em `unplaced`.
 */
export function arenaTeams(lobby: Lobby): ArenaLayout | null {
  const layout = lobby.subteams;
  if (!layout || layout.count <= 0 || layout.size <= 0) return null;
  const base = subteamBase(lobby.members);
  const teams: ArenaTeam[] = Array.from({ length: layout.count }, (_, t) => ({
    team: t + base,
    slots: Array.from({ length: layout.size }, (_, p) => ({ slot: { team: t + base, position: p + base }, member: null })),
  }));
  const unplaced: LobbyMember[] = [];
  for (const member of lobby.members) {
    const target = member.subteam && teams[member.subteam.team - base]?.slots[member.subteam.position - base];
    if (target && !target.member) target.member = member;
    else unplaced.push(member);
  }
  return { teams, unplaced };
}

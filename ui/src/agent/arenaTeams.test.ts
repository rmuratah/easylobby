import { describe, expect, it } from 'vitest';
import { arenaTeams, subteamBase } from './arenaTeams';
import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import { testLobby } from '../test/lobby';

const member = (puuid: string, subteam: LobbyMember['subteam']): LobbyMember => ({ ...testLobby.members[0], puuid, subteam });
const arena = (members: LobbyMember[]): Lobby => ({ ...testLobby, queueId: 1750, queue: null, maxLobbySize: 18, members, subteams: { count: 6, size: 3 } });

describe('equipes da Arena (#224)', () => {
  it('monta 6 equipes de 3 e põe cada jogador na vaga da LCU, contando de 1', () => {
    const layout = arenaTeams(arena([member('a', { team: 1, position: 1 }), member('b', { team: 3, position: 2 })]))!;
    expect(layout.teams).toHaveLength(6);
    expect(layout.teams.every(t => t.slots.length === 3)).toBe(true);
    expect(layout.teams[0].slots[0].member?.puuid).toBe('a');
    expect(layout.teams[2].slots[1]).toMatchObject({ slot: { team: 3, position: 2 }, member: { puuid: 'b' } });
    expect(layout.unplaced).toEqual([]);
  });

  it('conta de 0 quando a LCU manda um índice 0', () => {
    const members = [member('a', { team: 0, position: 0 })];
    expect(subteamBase(members)).toBe(0);
    const layout = arenaTeams(arena(members))!;
    expect(layout.teams[0].slots[0]).toMatchObject({ slot: { team: 0, position: 0 }, member: { puuid: 'a' } });
    expect(layout.teams[5].slots[2].slot).toEqual({ team: 5, position: 2 });
  });

  it('quem não tem vaga, está fora do layout ou repete uma vaga fica aguardando', () => {
    const layout = arenaTeams(arena([
      member('a', { team: 2, position: 1 }), member('b', { team: 2, position: 1 }), member('c', null), member('d', { team: 9, position: 1 }),
    ]))!;
    expect(layout.unplaced.map(m => m.puuid)).toEqual(['b', 'c', 'd']);
  });

  it('sem equipes no lobby, não monta nada', () => {
    expect(arenaTeams(testLobby)).toBeNull();
  });
});

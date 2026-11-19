import { describe, expect, it } from 'vitest';
import { testLobby } from '../test/lobby';
import { stageSeats } from './lobbyStage';

const member = (puuid: string) => ({ ...testLobby.members[0], puuid });
const names = (seats: ReturnType<typeof stageSeats>) => seats.map(seat => seat.kind === 'member' ? seat.member.puuid : `vaga-${seat.index}`);

describe('palco do lobby (#600)', () => {
  it('põe você no meio e os outros perto do centro, antes das vagas', () => {
    expect(names(stageSeats(member('eu'), [member('a'), member('b')], 2))).toEqual(['vaga-0', 'a', 'eu', 'b', 'vaga-1']);
  });

  it('com dois jogadores, o outro fica à esquerda', () => {
    expect(names(stageSeats(member('eu'), [member('a')], 0))).toEqual(['a', 'eu']);
  });

  it('sozinho numa fila de dois, a vaga fica ao lado', () => {
    expect(names(stageSeats(member('eu'), [], 1))).toEqual(['vaga-0', 'eu']);
  });

  it('sem você identificado, mantém a ordem do lobby', () => {
    expect(names(stageSeats(undefined, [member('a')], 1))).toEqual(['a', 'vaga-0']);
  });
});

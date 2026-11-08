import { describe, expect, it } from 'vitest';
import { GENERIC_RESTRICTION, restrictionReasons } from './lobbyRestrictions';
import { testLobby } from '../test/lobby';

describe('motivos da fila bloqueada (#227)', () => {
  it('mostra o texto do agente e o genérico para código sem texto, sem repetir', () => {
    const lobby = { ...testLobby, canStartActivity: false, restrictions: [
      { code: 'TeamSizeRestriction', message: 'Esta fila não aceita grupos deste tamanho.', puuids: [] },
      { code: 'Novo', message: null, puuids: ['a'] },
      { code: 'Novo', message: null, puuids: ['b'] },
    ] };
    expect(restrictionReasons(lobby)).toEqual(['Esta fila não aceita grupos deste tamanho.', `${GENERIC_RESTRICTION} (Novo)`]);
  });

  it('ignora os motivos com a fila liberada ou sem lobby', () => {
    const restrictions = [{ code: 'TeamSizeRestriction', message: 'x', puuids: [] }];
    expect(restrictionReasons({ ...testLobby, canStartActivity: true, restrictions })).toEqual([]);
    expect(restrictionReasons(null)).toEqual([]);
  });
});

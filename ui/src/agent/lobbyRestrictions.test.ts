import { describe, expect, it } from 'vitest';
import { GENERIC_RESTRICTION, pendingChoices, restrictionReasons } from './lobbyRestrictions';
import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
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

describe('escolhas que faltam antes da busca (#334)', () => {
  const nameOf = (member: LobbyMember) => `Jogador ${member.summonerId}`;
  const member = (overrides: Partial<LobbyMember>): LobbyMember => ({ ...testLobby.members[0], ...overrides });
  it('pede as posições primária e secundária de cada membro', () => {
    const lobby: Lobby = { ...testLobby, members: [
      member({}), member({ puuid: 'amigo', summonerId: 2, firstPosition: 'top' }), member({ puuid: 'outro', summonerId: 3, firstPosition: 'fill' }),
    ] };
    expect(pendingChoices(lobby, 'local', nameOf)).toEqual([
      'Escolha as posições primária e secundária.',
      'Jogador 2 ainda não escolheu as posições primária e secundária.',
    ]);
  });
  it('pede as duas posições e os campeões no Jogo Dinâmico', () => {
    const slot = { championId: 1, skinId: 0, position: 'top' as const, spell1Id: 4, spell2Id: 14, runeStyleId: 0, runeSubStyleId: 0, keystoneId: 0 };
    const lobby: Lobby = { ...testLobby, showPositionSelector: false, showQuickPlaySlots: true, members: [
      member({ playerSlots: [slot] }),
    ] };
    expect(pendingChoices(lobby, 'local', nameOf)).toEqual(['Escolha as duas posições e um campeão para cada uma.']);
  });
  it('não pede nada sem seletor, na ordem das 5 posições e na personalizada', () => {
    expect(pendingChoices({ ...testLobby, showPositionSelector: false }, 'local', nameOf)).toEqual([]);
    expect(pendingChoices({ ...testLobby, orderedPositions: true }, 'local', nameOf)).toEqual([]);
    expect(pendingChoices(null, 'local', nameOf)).toEqual([]);
  });
});

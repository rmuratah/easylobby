import { describe, expect, it } from 'vitest';
import { GENERIC_RESTRICTION, awayMembers, awayReasons, blockingMembers, pendingChoices, restrictionReasons } from './lobbyRestrictions';
import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import { testLobby } from '../test/lobby';

const nameOf = (member: LobbyMember) => `Jogador ${member.summonerId}`;

describe('motivos da fila bloqueada (#227)', () => {
  it('mostra o texto do agente e o genérico para código sem texto, sem repetir', () => {
    const lobby = { ...testLobby, canStartActivity: false, restrictions: [
      { code: 'TeamSizeRestriction', message: 'Esta fila não aceita grupos deste tamanho.', puuids: [] },
      { code: 'Novo', message: null, puuids: ['a'] },
      { code: 'Novo', message: null, puuids: ['b'] },
    ] };
    expect(restrictionReasons(lobby, 'local', nameOf)).toEqual(['Esta fila não aceita grupos deste tamanho.', `${GENERIC_RESTRICTION} (Novo)`]);
  });

  it('ignora os motivos com a fila liberada ou sem lobby', () => {
    const restrictions = [{ code: 'TeamSizeRestriction', message: 'x', puuids: [] }];
    expect(restrictionReasons({ ...testLobby, canStartActivity: true, restrictions }, 'local', nameOf)).toEqual([]);
    expect(restrictionReasons(null, 'local', nameOf)).toEqual([]);
  });
});

describe('quem está bloqueando a fila (#411)', () => {
  const members = [testLobby.members[0], { ...testLobby.members[0], puuid: 'amigo', summonerId: 2, isLeader: false },
    { ...testLobby.members[0], puuid: 'outro', summonerId: 3, isLeader: false }];
  const blocked = (restrictions: Lobby['restrictions']): Lobby => ({ ...testLobby, canStartActivity: false, members, restrictions });
  const punicao = 'Algum jogador está com punição de fila.';
  const elo = 'A diferença de elo entre os jogadores do grupo é grande demais.';

  it('troca "Algum jogador" pelo nome, ou por "Você"', () => {
    expect(restrictionReasons(blocked([{ code: 'PlayerLeaverBustedRestriction', message: punicao, puuids: ['amigo'] }]), 'local', nameOf))
      .toEqual(['Jogador 2 está com punição de fila.']);
    expect(restrictionReasons(blocked([{ code: 'PlayerLeaverBustedRestriction', message: punicao, puuids: ['local'] }]), 'local', nameOf))
      .toEqual(['Você está com punição de fila.']);
  });

  it('põe os nomes no fim com mais de um jogador ou em motivo do grupo', () => {
    expect(restrictionReasons(blocked([{ code: 'PlayerLeaverBustedRestriction', message: punicao, puuids: ['amigo', 'outro', 'local'] }]), 'local', nameOf))
      .toEqual(['Algum jogador está com punição de fila (Jogador 2, Jogador 3 e você).']);
    expect(restrictionReasons(blocked([{ code: 'TeamDivisionRestriction', message: elo, puuids: ['amigo'] }]), 'local', nameOf))
      .toEqual(['A diferença de elo entre os jogadores do grupo é grande demais (Jogador 2).']);
  });

  it('cita o jogador num código sem texto e ignora quem não está no grupo', () => {
    expect(restrictionReasons(blocked([{ code: 'Novo', message: null, puuids: ['amigo', 'fora'] }]), 'local', nameOf))
      .toEqual(['Jogador 2 não pode entrar nesta fila agora. (Novo)']);
    expect(restrictionReasons(blocked([{ code: 'PlayerLeaverBustedRestriction', message: punicao, puuids: ['fora'] }]), 'local', nameOf))
      .toEqual([punicao]);
  });

  it('junta os motivos de cada membro bloqueado, só com a fila bloqueada', () => {
    const lobby = blocked([
      { code: 'PlayerLeaverBustedRestriction', message: punicao, puuids: ['amigo'] },
      { code: 'TeamDivisionRestriction', message: elo, puuids: ['amigo', 'local'] },
      { code: 'TeamSizeRestriction', message: 'x', puuids: [] },
    ]);
    expect(blockingMembers(lobby)).toEqual(new Map([['amigo', [punicao, elo]], ['local', [elo]]]));
    expect(blockingMembers({ ...lobby, canStartActivity: true }).size).toBe(0);
    expect(blockingMembers(null).size).toBe(0);
  });
});

describe('escolhas que faltam antes da busca (#334)', () => {
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
    const slot = { championId: 1, skinId: 0, position: 'top' as const, spell1Id: 4, spell2Id: 14, runeStyleId: 0, runeSubStyleId: 0, keystoneId: 0, perkIds: [] };
    const lobby: Lobby = { ...testLobby, showPositionSelector: false, showQuickPlaySlots: true, members: [
      member({ playerSlots: [slot] }),
    ] };
    expect(pendingChoices(lobby, 'local', nameOf)).toEqual(['Escolha as duas posições e um campeão para cada uma.']);
  });
  it('pede as preferências de posição no Rift Clássico (#230)', () => {
    const lobby: Lobby = { ...testLobby, orderedPositions: true, members: [
      member({}), member({ puuid: 'amigo', summonerId: 2, positionOrder: ['fill'] }),
      member({ puuid: 'outro', summonerId: 3, positionOrder: ['middle', 'top', 'jungle', 'bottom', 'utility'] }),
    ] };
    expect(pendingChoices(lobby, 'local', nameOf)).toEqual(['Escolha as preferências de posição.']);
  });
  it('não pede nada sem seletor e na personalizada', () => {
    expect(pendingChoices({ ...testLobby, showPositionSelector: false }, 'local', nameOf)).toEqual([]);
    expect(pendingChoices(null, 'local', nameOf)).toEqual([]);
  });
});

describe('quem ainda não voltou ao lobby (#346)', () => {
  const member = (overrides: Partial<LobbyMember>): LobbyMember => ({ ...testLobby.members[0], ...overrides });
  const lobby: Lobby = { ...testLobby, members: [
    member({}), member({ puuid: 'amigo', summonerId: 2, ready: false }), member({ puuid: 'outro', summonerId: 3 }),
  ] };
  it('lista quem não está pronto só na fase de lobby', () => {
    expect(awayMembers(lobby, 'Lobby').map(m => m.puuid)).toEqual(['amigo']);
    expect(awayMembers(lobby, 'Matchmaking')).toEqual([]);
    expect(awayMembers(lobby, 'ChampSelect')).toEqual([]);
    expect(awayMembers(null, 'Lobby')).toEqual([]);
  });
  it('deixa a personalizada de fora', () => {
    const custom = { ...lobby, custom: {} as Lobby['custom'] };
    expect(awayMembers(custom, 'Lobby')).toEqual([]);
  });
  it('avisa pelo nome, e o próprio jogador como você', () => {
    const away = awayMembers({ ...lobby, members: [member({ ready: false }), member({ puuid: 'amigo', summonerId: 2, ready: false })] }, 'Lobby');
    expect(awayReasons(away, 'local', m => `Jogador ${m.summonerId}`)).toEqual([
      'Você ainda não voltou ao lobby.',
      'Jogador 2 ainda não voltou ao lobby.',
    ]);
  });
});

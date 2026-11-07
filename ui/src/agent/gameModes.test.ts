import { describe, expect, it } from 'vitest';
import type { GameQueue } from '../bindings/GameQueue';
import { buildModeTabs, firstAvailable, type ModeTab } from './gameModes';
import { lobbyQueueName } from './queueOptions';

const queue = (queueId: number, name: string, extra: Partial<GameQueue> = {}): GameQueue => ({
  queueId, queue: null, name, description: name, gameMode: 'CLASSIC', modeGroup: 'kOther', mapId: 11,
  versusAi: false, ranked: false, showPositionSelector: false, limitedTime: false, available: true, ...extra,
});
/** As filas como o agente lista na gravação 16.22, com a Arena 3x6 fechada. */
const recorded: GameQueue[] = [
  queue(400, 'Normal', { queue: 'normalDraft', description: 'Escolha alternada' }),
  queue(420, 'Ranqueada Solo/Duo', { queue: 'rankedSolo' }),
  queue(440, 'Ranqueada Flex', { queue: 'rankedFlex' }),
  queue(450, 'ARAM', { queue: 'aram' }),
  queue(480, 'Jogo Dinâmico', { queue: 'swiftplay' }),
  queue(870, 'Introdução', { versusAi: true }),
  queue(880, 'Iniciante', { versusAi: true }),
  queue(890, 'Intermediário', { versusAi: true }),
  queue(1750, 'Arena 3x6', { gameMode: 'CHERRY', available: false }),
  queue(2400, 'ARAM: Desordem', { gameMode: 'KIWI' }),
  queue(4310, '5v5 Clássico', { gameMode: 'JADE', description: '5v5 Clássico' }),
  queue(4320, 'Jade', { gameMode: 'JADE', versusAi: true, description: 'Clássico (Coop vs. IA)' }),
];
const tab = (tabs: ModeTab[], id: ModeTab['id']) => tabs.find(t => t.id === id)!;
const summary = (t: ModeTab) => t.modes.map(m => [m.name, m.available, m.submodes.map(s => [s.queueId, s.available])]);

describe('abas da tela de escolha de modo', () => {
  it('monta PvP, Co-op e Treinamento com os modos e submodos do mapa', () => {
    const tabs = buildModeTabs(recorded);
    expect(tabs.map(t => t.label)).toEqual(['PvP', 'Co-op versus IA', 'Treinamento']);
    expect(summary(tab(tabs, 'pvp'))).toEqual([
      ["Summoner's Rift", true, [[480, true], [400, true], [440, true], [420, true]]],
      ['ARAM', true, [[450, true], [2400, true]]],
      ['Arena', false, [[1750, false]]],
      ['Eventos', true, [[4310, true]]],
    ]);
    expect(summary(tab(tabs, 'coop'))).toEqual([
      ["Summoner's Rift", true, [[870, true], [880, true], [890, true]]],
      ['Eventos', true, [[4320, true]]],
    ]);
    const training = tab(tabs, 'training').modes;
    expect(training.map(m => [m.name, m.start])).toEqual([
      ['Tutorial', { type: 'startTutorial' }],
      ['Modo de Treino', { type: 'startPracticeTool' }],
    ]);
  });

  it('usa os nomes do design e o comando certo para cada fila', () => {
    const [sr, aram] = tab(buildModeTabs(recorded), 'pvp').modes;
    expect(sr.submodes.map(s => s.name)).toEqual(['Partida Dinâmica', 'Partida Alternada', 'Ranqueada Flex', 'Ranqueada Solo/Duo']);
    expect(sr.submodes[0].command).toEqual({ type: 'createLobby', queue: 'swiftplay' });
    expect(aram.submodes.map(s => [s.name, s.command])).toEqual([
      ['ARAM Normal', { type: 'createLobby', queue: 'aram' }],
      ['ARAM Desordem', { type: 'createLobbyById', queueId: 2400 }],
    ]);
  });

  it('sem fila de evento aberta, não mostra o card Eventos', () => {
    const tabs = buildModeTabs(recorded.filter(q => q.queueId < 4000));
    expect(tab(tabs, 'pvp').modes.map(m => m.id)).toEqual(['summonersRift', 'aram', 'arena']);
    expect(tab(tabs, 'coop').modes.map(m => m.id)).toEqual(['coopSummonersRift']);
    // Evento fechado também não conta.
    const closed = buildModeTabs([...recorded.filter(q => q.queueId < 4000), queue(4310, 'Jade', { available: false })]);
    expect(tab(closed, 'pvp').modes.map(m => m.id)).not.toContain('events');
  });

  it('acinzenta o modo fixo fechado e libera a Arena 3x6 quando abre', () => {
    const open = buildModeTabs(recorded.map(q => q.queueId === 1750 ? { ...q, available: true } : q));
    const arena = tab(open, 'pvp').modes.find(m => m.id === 'arena')!;
    expect(arena.available).toBe(true);
    expect(arena.submodes[0].command).toEqual({ type: 'createLobbyById', queueId: 1750 });
    const flexClosed = buildModeTabs(recorded.map(q => q.queueId === 440 ? { ...q, available: false } : q));
    const sr = tab(flexClosed, 'pvp').modes[0];
    expect(sr.available).toBe(true);
    expect(sr.submodes.find(s => s.queueId === 440)!.available).toBe(false);
  });

  it('antes da lista chegar, deixa tentar todos os modos fixos', () => {
    const tabs = buildModeTabs([]);
    expect(summary(tab(tabs, 'pvp'))).toEqual([
      ["Summoner's Rift", true, [[480, true], [400, true], [440, true], [420, true]]],
      ['ARAM', true, [[450, true], [2400, true]]],
      ['Arena', true, [[1750, true]]],
    ]);
    expect(tab(tabs, 'coop').modes[0].available).toBe(true);
    expect(firstAvailable(tab(tabs, 'coop'))?.submode?.queueId).toBe(870);
  });

  it('a seleção inicial é o primeiro submodo aberto da aba', () => {
    const tabs = buildModeTabs(recorded);
    expect(firstAvailable(tab(tabs, 'pvp'))?.submode?.queueId).toBe(480);
    expect(firstAvailable(tab(tabs, 'coop'))?.submode?.queueId).toBe(870);
    expect(firstAvailable(tab(tabs, 'training'))?.mode.id).toBe('tutorial');
  });

  it('dá nome ao lobby do Tutorial e do Modo de Treino', () => {
    expect(lobbyQueueName({ queueId: 2010, queue: null }, recorded)).toBe('Tutorial, Parte 2');
    expect(lobbyQueueName({ queueId: 3140, queue: null }, recorded)).toBe('Modo de Treino');
  });
});

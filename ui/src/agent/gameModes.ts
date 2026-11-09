import type { Command } from '../bindings/Command';
import type { GameQueue } from '../bindings/GameQueue';
import type { Queue } from '../bindings/Queue';
import { createLobbyCommand, queueKey } from './queueOptions';

/**
 * Tela de escolha de modo de jogo (#210): abas, modos (cards) e submodos
 * (filas). Os modos fixos e os ids das filas seguem o mapa da #209/#210
 * (gravação 16.22); o card Summoner's Rift Clássico (antes Eventos, #280) junta
 * as outras filas abertas que o agente lista.
 */
export type ModeTabId = 'pvp' | 'coop' | 'training' | 'custom';
export type ModeId = 'summonersRift' | 'aram' | 'arena' | 'events' | 'coopSummonersRift' | 'coopEvents' | 'tutorial' | 'practiceTool';
export type CreateLobby = Extract<Command, { type: 'createLobby' | 'createLobbyById' }>;
export type TrainingCommand = Extract<Command, { type: 'startTutorial' | 'startPracticeTool' }>;

export type Submode = {
  queueId: number;
  name: string;
  description: string;
  /** Fechada pela Riot: aparece acinzentada e não pode ser escolhida. */
  available: boolean;
  /** Fila em que o lobby já está (#239): aparece bloqueada na tela de trocar modo. */
  current?: boolean;
  command: CreateLobby;
};
export type GameMode = {
  id: ModeId;
  name: string;
  description: string;
  /** Sigla no lugar do ícone, até as imagens finais entrarem. */
  icon: string;
  /** Algum submodo aberto (ou, no Treinamento, sempre). */
  available: boolean;
  submodes: Submode[];
  /** Só no Treinamento: o comando do botão de confirmar. */
  start?: TrainingCommand;
};
export type ModeTab = { id: ModeTabId; label: string; modes: GameMode[] };

type FixedSubmode = { queueId: number; queue?: Queue; name: string; description: string };
type FixedMode = { id: ModeId; name: string; description: string; icon: string; submodes: FixedSubmode[] };

const pvpModes: FixedMode[] = [
  {
    id: 'summonersRift', name: "Summoner's Rift", icon: 'SR', description: 'O mapa clássico: 5 contra 5 em três rotas.',
    submodes: [
      { queueId: 480, queue: 'swiftplay', name: 'Partida Dinâmica', description: 'Partidas mais curtas, com campeões e cargos escolhidos antes da fila.' },
      { queueId: 400, queue: 'normalDraft', name: 'Partida Alternada', description: 'Bans e escolhas alternadas, sem valer ranque.' },
      { queueId: 440, queue: 'rankedFlex', name: 'Ranqueada Flex', description: 'Ranqueada para grupos de qualquer tamanho.' },
      { queueId: 420, queue: 'rankedSolo', name: 'Ranqueada Solo/Duo', description: 'Ranqueada sozinho ou em dupla.' },
    ],
  },
  {
    id: 'aram', name: 'ARAM', icon: 'AR', description: 'Uma rota só, campeões aleatórios, no Abismo Uivante.',
    submodes: [
      { queueId: 450, queue: 'aram', name: 'ARAM Normal', description: 'O ARAM de sempre.' },
      { queueId: 2400, name: 'ARAM Desordem', description: 'ARAM com aprimoramentos ao longo da partida.' },
    ],
  },
  {
    id: 'arena', name: 'Arena', icon: 'AN', description: 'Combates rápidos por rodadas até sobrar um time.',
    submodes: [{ queueId: 1750, name: 'Arena 3x6', description: 'Seis trios disputam rodada a rodada.' }],
  },
];

const coopModes: FixedMode[] = [
  {
    id: 'coopSummonersRift', name: "Summoner's Rift", icon: 'SR', description: 'Você e seu time contra bots no mapa clássico.',
    submodes: [
      { queueId: 870, name: 'Introdução', description: 'Bots bem fáceis, para conhecer o jogo.' },
      { queueId: 880, name: 'Iniciante', description: 'Bots para quem está começando.' },
      { queueId: 890, name: 'Intermediário', description: 'Bots mais desafiadores.' },
    ],
  },
];

const trainingModes: GameMode[] = [
  {
    id: 'tutorial', name: 'Tutorial', icon: 'TU', available: true, submodes: [], start: { type: 'startTutorial' },
    description: 'Aprenda o básico em três partes. Escolha a parte para jogar.',
  },
  {
    id: 'practiceTool', name: 'Modo de Treino', icon: 'MT', available: true, submodes: [], start: { type: 'startPracticeTool' },
    description: "Teste campeões e combos sem pressão no Summoner's Rift.",
  },
];

/** `queueId` com lugar fixo na tela; o resto das filas abertas vai para o card Summoner's Rift Clássico (a Arena, para o card Arena). */
const fixedQueueIds = new Set([...pvpModes, ...coopModes].flatMap(mode => mode.submodes.map(s => s.queueId)));

function fixedMode(mode: FixedMode, queues: GameQueue[]): GameMode {
  const submodes = mode.submodes.map((fixed): Submode => {
    const listed = queues.find(q => q.queueId === fixed.queueId);
    // Só fica cinza o que o agente disse que está fechado. Fila que ainda não veio na lista (lida antes do
    // login, por exemplo) dá para tentar: o agente repassa e a LCU decide.
    const available = listed ? listed.available : true;
    const command = createLobbyCommand(listed ? queueKey(listed) : fixed.queue ?? `fila-${fixed.queueId}`);
    return { queueId: fixed.queueId, name: fixed.name, description: fixed.description, available, command };
  });
  // As outras Arenas abertas (Arena da Bravura, #154) entram no card Arena, depois da fixa.
  if (mode.id === 'arena') submodes.push(...queues.filter(q => isOtherArena(q)).map((q): Submode => ({
    queueId: q.queueId,
    name: q.name || q.description || `Arena ${q.queueId}`,
    description: q.description && q.description !== q.name ? q.description : 'Arena por tempo limitado.',
    available: true,
    command: createLobbyCommand(queueKey(q)),
  })));
  return { id: mode.id, name: mode.name, description: mode.description, icon: mode.icon, available: submodes.some(s => s.available), submodes };
}

/** `gameMode` da Arena na LCU. */
const ARENA_GAME_MODE = 'CHERRY';

/** Arena aberta sem lugar fixo na tela (#154): vai para o card Arena, não para o de eventos. */
function isOtherArena(q: GameQueue) {
  return q.available && !q.versusAi && q.gameMode === ARENA_GAME_MODE && !fixedQueueIds.has(q.queueId);
}

type EventsCard = Pick<GameMode, 'name' | 'icon' | 'description'> & {
  /** Usa a descrição da LCU como nome da fila (o nome dela vem interno, como "Jade"). */
  nameFromDescription?: boolean;
  /** Descrição curta no lugar da da LCU. */
  submodeDescription?: string;
};

/** Card das filas sem lugar fixo: hoje são as filas do 5v5 clássico (#280). */
const eventsCards: Record<'events' | 'coopEvents', EventsCard> = {
  events: { name: "Summoner's Rift Clássico", icon: 'SR', description: 'O 5 contra 5 clássico e as outras filas abertas agora.' },
  coopEvents: {
    name: "Summoner's Rift Clássico", icon: 'SR', description: 'O 5 contra 5 clássico contra bots.',
    nameFromDescription: true, submodeDescription: 'Seu time contra bots.',
  },
};

function eventsMode(id: 'events' | 'coopEvents', queues: GameQueue[]): GameMode | null {
  const { nameFromDescription, submodeDescription, ...card } = eventsCards[id];
  const submodes = queues.filter(q => q.available && !fixedQueueIds.has(q.queueId) && !isOtherArena(q)).map((q): Submode => ({
    queueId: q.queueId,
    name: (nameFromDescription && q.description) || q.name || q.description || `Fila ${q.queueId}`,
    description: submodeDescription ?? (q.description && q.description !== q.name ? q.description : 'Modo por tempo limitado.'),
    available: true,
    command: createLobbyCommand(queueKey(q)),
  }));
  if (!submodes.length) return null;
  return { id, ...card, available: true, submodes };
}

/** Monta as abas a partir da lista de filas do agente. */
export function buildModeTabs(queues: GameQueue[]): ModeTab[] {
  const pvpEvents = eventsMode('events', queues.filter(q => !q.versusAi));
  const coopEvents = eventsMode('coopEvents', queues.filter(q => q.versusAi));
  return [
    { id: 'pvp', label: 'PvP', modes: [...pvpModes.map(m => fixedMode(m, queues)), ...(pvpEvents ? [pvpEvents] : [])] },
    { id: 'coop', label: 'Co-op versus IA', modes: [...coopModes.map(m => fixedMode(m, queues)), ...(coopEvents ? [coopEvents] : [])] },
    { id: 'training', label: 'Treinamento', modes: trainingModes },
    // Partida personalizada (#262): a aba mostra o formulário, sem cards de modo.
    { id: 'custom', label: 'Personalizada', modes: [] },
  ];
}

/** Primeiro submodo aberto da aba, para a seleção inicial. */
export function firstAvailable(tab: ModeTab): { mode: GameMode; submode: Submode | null } | null {
  for (const mode of tab.modes) {
    if (!mode.available) continue;
    if (!mode.submodes.length) return { mode, submode: null };
    const submode = mode.submodes.find(s => s.available);
    if (submode) return { mode, submode };
  }
  return null;
}

/**
 * Na tela de trocar modo (#239), a fila atual do lobby não pode ser escolhida:
 * o submodo fica fechado e marcado como atual, e o modo só continua aberto se
 * tiver outra fila aberta.
 */
export function blockCurrentQueue(tabs: ModeTab[], queueId: number): ModeTab[] {
  return tabs.map(tab => ({
    ...tab,
    modes: tab.modes.map(mode => {
      if (!mode.submodes.some(s => s.queueId === queueId)) return mode;
      const submodes = mode.submodes.map(s => s.queueId === queueId ? { ...s, available: false, current: true } : s);
      return { ...mode, available: submodes.some(s => s.available), submodes };
    }),
  }));
}

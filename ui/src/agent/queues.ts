import type { GameQueue } from '../bindings/GameQueue';
import type { Queue } from '../bindings/Queue';

export const queueNames = {
  normalDraft: 'Normal — Escolha alternada',
  swiftplay: 'Jogo Dinâmico',
  rankedSolo: 'Ranqueada Solo/Duo',
  rankedFlex: 'Ranqueada Flex',
  aram: 'ARAM',
} satisfies Record<Queue, string>;

export function isQueue(value: unknown): value is Queue {
  return typeof value === 'string' && Object.hasOwn(queueNames, value);
}

/** Modos de ARAM na LCU: o ARAM e a ARAM: Desordem (`KIWI`). */
const aramModes = new Set(['ARAM', 'KIWI']);
/** `queueId` dos ARAM, para quando a fila não está na lista do agente (ARAM e ARAM: Desordem). */
const aramQueueIds = new Set([450, 2400]);

/** Se a fila é um ARAM, em que o easylobby não mostra a troca de runas. */
export function isAramQueue(current: { queueId: number; queue: Queue | null }, queues: GameQueue[]): boolean {
  if (current.queue === 'aram' || aramQueueIds.has(current.queueId)) return true;
  const listed = queues.find(q => q.queueId === current.queueId);
  return !!listed && (aramModes.has(listed.gameMode) || listed.modeGroup === 'kARAM');
}

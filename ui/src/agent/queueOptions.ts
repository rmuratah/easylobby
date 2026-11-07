import type { Command } from '../bindings/Command';
import type { GameQueue } from '../bindings/GameQueue';
import type { Queue } from '../bindings/Queue';
import { isQueue, queueNames } from './queues';

/**
 * Chave de uma fila no seletor: o nome da [`Queue`] fixa (`aram`,
 * `rankedSolo`...) ou `fila-<queueId>` nas filas que vêm só da lista da LCU
 * (ARAM: Desordem, Jade, contra IA..., #146).
 */
export type QueueKey = Queue | `fila-${number}`;

export function queueKey(queue: { queueId: number; queue: Queue | null }): QueueKey {
  return queue.queue ?? `fila-${queue.queueId}`;
}

export type QueueOption = { key: QueueKey; label: string };
export type QueueGroup = { label: string; options: QueueOption[] };

/**
 * Filas do seletor, em grupos. As cinco fixas aparecem sempre (como antes);
 * as outras filas abertas que o agente lê da LCU entram depois, separando
 * os modos contra jogadores dos contra IA.
 */
export function queueGroups(queues: GameQueue[]): QueueGroup[] {
  const fixed: QueueOption[] = (Object.keys(queueNames) as Queue[]).map(queue => ({ key: queue, label: queueNames[queue] }));
  const extra = queues.filter(q => q.queue === null);
  const option = (q: GameQueue): QueueOption => ({ key: queueKey(q), label: q.name || q.description || `Fila ${q.queueId}` });
  const groups: QueueGroup[] = [{ label: 'Filas principais', options: fixed }];
  const pvp = extra.filter(q => !q.versusAi).map(option);
  const bots = extra.filter(q => q.versusAi).map(option);
  if (pvp.length) groups.push({ label: 'Mais modos', options: pvp });
  if (bots.length) groups.push({ label: 'Contra IA', options: bots });
  return groups;
}

/** Comando que cria o lobby (ou troca a fila) para a chave escolhida. */
export function createLobbyCommand(key: QueueKey): Extract<Command, { type: 'createLobby' | 'createLobbyById' }> {
  return isQueue(key) ? { type: 'createLobby', queue: key } : { type: 'createLobbyById', queueId: Number(key.slice('fila-'.length)) };
}

export function isQueueKey(value: string, groups: QueueGroup[]): value is QueueKey {
  return groups.some(group => group.options.some(option => option.key === value));
}

/** Nome da fila de um lobby: o nome fixo, o da lista da LCU, ou o id. */
export function lobbyQueueName(lobby: { queueId: number; queue: Queue | null }, queues: GameQueue[]): string {
  if (lobby.queue) return queueNames[lobby.queue];
  const listed = queues.find(q => q.queueId === lobby.queueId);
  return listed?.name || `Outra fila (ID ${lobby.queueId})`;
}

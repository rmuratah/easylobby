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

/** Comando que cria o lobby (ou troca a fila) para a chave escolhida. */
export function createLobbyCommand(key: QueueKey): Extract<Command, { type: 'createLobby' | 'createLobbyById' }> {
  return isQueue(key) ? { type: 'createLobby', queue: key } : { type: 'createLobbyById', queueId: Number(key.slice('fila-'.length)) };
}

/** Filas fora da lista do agente que a aba Treinamento cria (Tutorial, Modo de Treino, #210). */
const trainingQueueNames: Record<number, string> = {
  2000: 'Tutorial, Parte 1',
  2010: 'Tutorial, Parte 2',
  2020: 'Tutorial, Parte 3',
  3140: 'Modo de Treino',
};

/** Nome da fila de um lobby: o nome fixo, o da lista da LCU, ou o id. */
export function lobbyQueueName(lobby: { queueId: number; queue: Queue | null }, queues: GameQueue[]): string {
  if (lobby.queue) return queueNames[lobby.queue];
  const listed = queues.find(q => q.queueId === lobby.queueId);
  return listed?.name || trainingQueueNames[lobby.queueId] || `Outra fila (ID ${lobby.queueId})`;
}

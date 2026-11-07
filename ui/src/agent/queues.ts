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

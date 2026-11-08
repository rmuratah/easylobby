import type { TutorialPart } from '../bindings/TutorialPart';

/** Filas das Partes 1, 2 e 3 do Tutorial (#209). */
const TUTORIAL_QUEUE_IDS = [2000, 2010, 2020] as const;

/** Se o lobby é de uma parte do Tutorial, que não tem lobby aberto nem link (#225). */
export function isTutorialQueue(queueId: number): boolean {
  return (TUTORIAL_QUEUE_IDS as readonly number[]).includes(queueId);
}

/** Os cards das partes do Tutorial, como no client oficial (#228). */
const tutorialPartTexts: Record<number, { name: string; description: string }> = {
  2000: { name: 'Bem-vindo(a) ao League of Legends', description: 'Aprenda a se mover, atacar e usar as habilidades.' },
  2010: { name: 'Fortalecimento', description: 'Aprenda a subir de nível e a fortalecer as habilidades.' },
  2020: { name: 'Comprando equipamentos', description: 'Aprenda a comprar itens na loja e a usá-los na partida.' },
};

/** Recompensa de cada parte do Tutorial, em EXP. */
export const TUTORIAL_REWARD_XP = 150;

/** Aviso das partes com cadeado. */
export const TUTORIAL_LOCKED = 'Conclua a partida de tutorial anterior para habilitar';

/**
 * As três partes com o progresso do agente. Sem o progresso (`null`), todas
 * ficam liberadas e a LCU decide ao criar o lobby.
 */
export function tutorialCards(parts: TutorialPart[] | null) {
  return TUTORIAL_QUEUE_IDS.map((queueId, index) => {
    const part = parts?.find(p => p.queueId === queueId);
    return { queueId, part: index + 1, ...tutorialPartTexts[queueId],
      completed: part?.completed ?? false, available: part ? part.available : true };
  });
}
export type TutorialCard = ReturnType<typeof tutorialCards>[number];

/** Parte escolhida de início: a primeira liberada que falta concluir, ou a Parte 1. */
export function defaultTutorialPart(cards: TutorialCard[]): number {
  return cards.find(c => c.available && !c.completed)?.queueId ?? TUTORIAL_QUEUE_IDS[0];
}

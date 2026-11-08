/** Filas das Partes 1, 2 e 3 do Tutorial (#209). */
export const TUTORIAL_QUEUE_IDS = [2000, 2010, 2020] as const;

/** Se o lobby é de uma parte do Tutorial, que não tem lobby aberto nem link (#225). */
export function isTutorialQueue(queueId: number): boolean {
  return (TUTORIAL_QUEUE_IDS as readonly number[]).includes(queueId);
}

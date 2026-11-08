import type { BotDifficulty } from '../bindings/BotDifficulty';
import type { Position } from '../bindings/Position';

/** As 5 rotas de um bot, sem "Recomendados" e "Outras funções" (#229). */
export const BOT_POSITIONS = ['top', 'jungle', 'middle', 'bottom', 'utility'] as const satisfies readonly Position[];

/** Dificuldades de bot, como o client oficial mostra. */
export const botDifficultyNames = {
  intro: 'Introdução',
  beginner: 'Iniciante',
  intermediate: 'Intermediário',
} satisfies Record<Exclude<BotDifficulty, 'unknown'>, string>;

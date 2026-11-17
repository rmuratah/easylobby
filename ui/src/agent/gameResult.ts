import type { GameQueue } from '../bindings/GameQueue';
import type { GameResult } from '../bindings/GameResult';
import type { MatchPlayer } from '../bindings/MatchPlayer';
import type { RankedChange } from '../bindings/RankedChange';
import { queueLabel, rankLabel } from './profile';

/** Título do resultado, do ponto de vista do jogador logado. */
export const resultTitle = { win: 'Vitória', loss: 'Derrota', remake: 'Remake' } as const;

const modeNames: Record<string, string> = {
  CLASSIC: "Summoner's Rift",
  ARAM: 'ARAM',
  CHERRY: 'Arena',
  URF: 'URF',
  TUTORIAL: 'Tutorial',
  PRACTICETOOL: 'Ferramenta de Treino',
};

/**
 * Nome da fila: o que a LCU mandou, depois a lista conhecida pelo `queueId`;
 * sem fila nenhuma (o agente não leu a sessão), o modo de jogo.
 */
export function resultQueue(result: GameResult, queues: GameQueue[]): string {
  if (result.queueName) return result.queueName;
  if (result.queueId > 0) return queueLabel(result.queueId, queues);
  return modeNames[result.gameMode] ?? (result.gameMode || 'Partida');
}

/** O jogador logado no placar, ou `null` se o bloco não marcou ninguém. */
export function localPlayer(result: GameResult): MatchPlayer | null {
  if (!result.localPuuid) return null;
  for (const team of result.teams) {
    const me = team.players.find(p => p.puuid === result.localPuuid);
    if (me) return me;
  }
  return null;
}

/**
 * A mudança de elo em texto: `+64 PDL`, `-18 PDL`, `Promovido a Ouro I` ou
 * `Rebaixado para Prata IV`. Numa mudança de tier ou divisão, a direção sai
 * da partida (vitória sobe, derrota desce).
 */
export function rankedChangeText(change: RankedChange, won: boolean): string {
  const delta = change.leaguePointsDelta;
  if (delta !== null) return `${delta > 0 ? '+' : ''}${delta} PDL`;
  const rank = rankLabel(change.after);
  if (!change.before.tier) return `Classificado em ${rank}`;
  return won ? `Promovido a ${rank}` : `Rebaixado para ${rank}`;
}

/** Abates por minuto, com uma casa (`6,4`). */
export function perMinute(value: number, durationSecs: number): string {
  if (durationSecs <= 0) return '0';
  return (value / (durationSecs / 60)).toFixed(1).replace('.', ',');
}

/** `9.127` -> `9,1 mil`; abaixo de mil, o número inteiro. */
export function compactNumber(value: number): string {
  if (value < 1000) return String(value);
  return `${(value / 1000).toFixed(1).replace('.', ',')} mil`;
}

import { describe, expect, it } from 'vitest';
import { lowPriorityText, matchmakingErrorText } from './matchmaking';

const error = (errorType: string, penaltyTimeRemaining = 0, message = '') => ({ errorType, message, penaltyTimeRemaining });

describe('matchmakingErrorText (#502)', () => {
  it('troca o código da punição por texto com o tempo', () => {
    expect(matchmakingErrorText(error('QUEUE_DODGER', 42)))
      .toBe('Você saiu de uma seleção de campeões e precisa esperar 0:42 para entrar na fila.');
    expect(matchmakingErrorText(error('LEAVER_BUSTED', 600)))
      .toBe('Você abandonou ou ficou ausente em partidas recentes e precisa esperar 10:00 para entrar na fila.');
  });

  it('sem tempo, só o motivo', () => {
    expect(matchmakingErrorText(error('RANKED_MIN_LEVEL'))).toBe('Sua conta ainda não tem o nível necessário para as ranqueadas.');
  });

  it('código desconhecido usa a mensagem da Riot ou uma frase genérica', () => {
    expect(matchmakingErrorText(error('NOVO_CODIGO', 0, 'Algo aconteceu'))).toBe('Algo aconteceu.');
    expect(matchmakingErrorText(error('NOVO_CODIGO', 90)))
      .toBe('O League of Legends não deixou entrar na fila agora (NOVO_CODIGO). Aguarde 1:30 para tentar de novo.');
    expect(matchmakingErrorText(error(''))).toBe('O League of Legends não deixou entrar na fila agora.');
  });

  it('baixa prioridade em frase', () => {
    expect(lowPriorityText(120)).toBe('Você abandonou partidas recentes e vai esperar 2:00 na fila de baixa prioridade antes da busca começar.');
  });
});

import type { ReadyCheckState } from '../bindings/ReadyCheckState';
import type { ReadyCheckResponse } from '../bindings/ReadyCheckResponse';

export const readyCheckLabels = {
  InProgress: 'Aguardando respostas', EveryoneReady: 'Todos aceitaram',
  StrangerNotReady: 'Alguém de fora do grupo não aceitou', PartyNotReady: 'Alguém do grupo não aceitou',
  Error: 'Erro ao confirmar a partida', Invalid: 'Partida indisponível', Unknown: 'Estado da partida desconhecido',
} satisfies Record<ReadyCheckState, string>;
export const responseLabels = {
  None: 'Ainda não respondeu', Accepted: 'Aceitou', Declined: 'Recusou', Unknown: 'Resposta desconhecida',
} satisfies Record<ReadyCheckResponse, string>;


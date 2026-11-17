import type { ConnectionSnapshot } from './client';
import type { Feature } from '../bindings/Feature';
import type { UnavailableFeature } from '../bindings/UnavailableFeature';

/** Nome de cada função no aviso do modo de compatibilidade (#36). */
export const featureLabels: Record<Feature, string> = {
  lobby: 'Lobby e fila',
  readyCheck: 'Partida encontrada',
  champSelect: 'Seleção de campeões',
  runes: 'Runas',
  social: 'Amigos e conversas',
  profile: 'Perfil',
  postGame: 'Fim de partida',
};

/** Funções desligadas pelo agente; vazio sem conexão com o LoL. */
export function unavailableFeatures(connection: ConnectionSnapshot): UnavailableFeature[] {
  if (connection.status !== 'connected' || connection.state?.lcu.state !== 'connected') return [];
  return connection.state.unavailableFeatures;
}

export function unavailableFeature(connection: ConnectionSnapshot, feature: Feature): UnavailableFeature | null {
  return unavailableFeatures(connection).find(u => u.feature === feature) ?? null;
}

/** O que aconteceu, em linguagem simples, para a tela e para quem reportar. */
export function problemText(item: UnavailableFeature): string {
  return item.problem === 'missing'
    ? 'O LoL deixou de responder a uma parte que o easylobby usa.'
    : 'O LoL passou a responder num formato que o easylobby ainda não entende.';
}

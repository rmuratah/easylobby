import type { CustomGameOptions } from '../bindings/CustomGameOptions';
import type { CustomGameSettings } from '../bindings/CustomGameSettings';
import type { CustomMap } from '../bindings/CustomMap';
import type { CustomPickType } from '../bindings/CustomPickType';
import type { SpectatorPolicy } from '../bindings/SpectatorPolicy';

/**
 * Partida personalizada (#262 e #263): nomes das opções do formulário, como o
 * client oficial mostra, e o resumo do cabeçalho do lobby. Os limites seguem o
 * agente: nome até 64 caracteres e equipe de 1 a 5.
 */
export const CUSTOM_NAME_MAX = 64;
export const TEAM_SIZES = [1, 2, 3, 4, 5] as const;

export const customMapNames = {
  summonersRift: "Summoner's Rift",
  howlingAbyss: 'Abismo Uivante',
} satisfies Record<CustomMap, string>;

export const customMapDescriptions = {
  summonersRift: 'O mapa clássico, com três rotas.',
  howlingAbyss: 'ARAM: uma rota só.',
} satisfies Record<CustomMap, string>;

export const pickTypeNames = {
  blindPick: 'Escolha às cegas',
  draftMode: 'Escolha alternada',
  allRandom: 'Todos aleatórios',
  tournamentDraft: 'Alternada para torneios',
} satisfies Record<CustomPickType, string>;

export const spectatorNames = {
  none: 'Ninguém',
  lobbyOnly: 'Só quem está no lobby',
  friendsOnly: 'Só amigos',
  all: 'Todos',
} satisfies Record<SpectatorPolicy, string>;

/** O formulário já preenchido com os padrões do client oficial. */
export function defaultCustomOptions(summonerName: string | undefined): CustomGameOptions {
  return {
    name: summonerName ? `Partida de ${summonerName}`.slice(0, CUSTOM_NAME_MAX) : 'Partida personalizada',
    password: '',
    map: 'summonersRift',
    pickType: 'blindPick',
    teamSize: 5,
    spectators: 'lobbyOnly',
  };
}

/** Por que o formulário não pode ser enviado, ou `null` quando pode. */
export function customOptionsProblem(options: CustomGameOptions): string | null {
  const name = options.name.trim();
  if (!name) return 'Dê um nome para a partida.';
  if ([...name].length > CUSTOM_NAME_MAX) return `O nome pode ter até ${CUSTOM_NAME_MAX} caracteres.`;
  return null;
}

/** Linha de resumo: "Summoner's Rift · 5v5 · Escolha alternada". */
export function customOptionsSummary(options: Pick<CustomGameOptions, 'map' | 'pickType' | 'teamSize'>): string {
  return [customMapNames[options.map], `${options.teamSize}v${options.teamSize}`, pickTypeNames[options.pickType]].join(' · ');
}

const PRACTICE_TOOL = 'PRACTICETOOL';

/** Título do lobby de personalizada: o nome da partida, ou "Modo de Treino". */
export function customLobbyTitle(settings: CustomGameSettings): string {
  if (settings.gameMode === PRACTICE_TOOL) return 'Modo de Treino';
  return settings.name.trim() || 'Partida personalizada';
}

/** Resumo do cabeçalho do lobby, só com o que o agente soube ler. */
export function customLobbySummary(settings: CustomGameSettings, teamSize: number): string {
  return [
    settings.map ? customMapNames[settings.map] : null,
    `${teamSize}v${teamSize}`,
    settings.pickType ? pickTypeNames[settings.pickType] : null,
    settings.gameMode === PRACTICE_TOOL ? 'Modo de Treino' : null,
  ].filter(Boolean).join(' · ');
}

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
  classicRift: 'Rift Clássico',
} satisfies Record<CustomMap, string>;

/**
 * Modos da aba Personalizada, na ordem do client oficial (#270). O Mapa
 * aleatório, com o Abismo Uivante, entra no #271.
 */
export const FORM_MAPS = ['summonersRift', 'classicRift'] as const satisfies readonly CustomMap[];

/** Tipos de jogo de cada modo, na ordem e com os nomes do client oficial. */
export const pickTypesByMap: Record<CustomMap, readonly CustomPickType[]> = {
  summonersRift: ['blindPick', 'draftMode', 'allRandom', 'tournamentDraft'],
  howlingAbyss: ['allRandom', 'blindPick', 'tournamentDraft', 'draftMode'],
  classicRift: ['draftMode', 'blindPick'],
};

const pickTypeNames = {
  blindPick: 'Escolha às Cegas',
  draftMode: 'Modo alternado',
  allRandom: 'Todos aleatórios',
  tournamentDraft: 'Alternada Torneio',
} satisfies Record<CustomPickType, string>;

/** O Rift Clássico chama a alternada de "Escolha Alternada". */
export function pickTypeName(map: CustomMap | null, pickType: CustomPickType): string {
  return map === 'classicRift' && pickType === 'draftMode' ? 'Escolha Alternada' : pickTypeNames[pickType];
}

/** Ordem do client oficial: duas colunas, saguão e amigos em cima. */
export const spectatorNames = {
  lobbyOnly: 'Somente do saguão',
  friendsOnly: 'Somente da lista de amigos',
  all: 'Todos',
  none: 'Nenhum',
} satisfies Record<SpectatorPolicy, string>;

/** O formulário já preenchido com os padrões do client oficial. */
export function defaultCustomOptions(summonerName: string | undefined): CustomGameOptions {
  return {
    name: summonerName ? `Partida de ${summonerName}`.slice(0, CUSTOM_NAME_MAX) : 'Partida personalizada',
    password: '',
    map: 'summonersRift',
    pickType: 'blindPick',
    teamSize: 5,
    spectators: 'all',
    hideFromPublicList: false,
    spectatorDelay: true,
  };
}

/** Troca o modo: o tipo de jogo vira o primeiro do modo quando o atual não existe nele. */
export function withMap(options: CustomGameOptions, map: CustomMap): CustomGameOptions {
  const pickTypes = pickTypesByMap[map];
  return { ...options, map, pickType: pickTypes.includes(options.pickType) ? options.pickType : pickTypes[0] };
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
  return [customMapNames[options.map], `${options.teamSize}v${options.teamSize}`, pickTypeName(options.map, options.pickType)].join(' · ');
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
    settings.pickType ? pickTypeName(settings.map, settings.pickType) : null,
    settings.gameMode === PRACTICE_TOOL ? 'Modo de Treino' : null,
  ].filter(Boolean).join(' · ');
}

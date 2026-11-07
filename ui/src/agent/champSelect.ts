import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectAction } from '../bindings/ChampSelectAction';
import type { ChampSelectPhase } from '../bindings/ChampSelectPhase';
import type { ChampSelectPlayer } from '../bindings/ChampSelectPlayer';
import type { Champion } from '../bindings/Champion';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';

export const champSelectPhaseLabels = {
  PLANNING: 'Planejamento',
  BAN_PICK: 'Bans e escolhas',
  FINALIZATION: 'Finalização',
  GAME_STARTING: 'Partida abrindo',
  UNKNOWN: 'Etapa desconhecida',
} satisfies Record<ChampSelectPhase, string>;

/** Nome do campeão no idioma do client, ou um nome provisório sem o catálogo. */
export function championName(catalog: ChampionCatalog | null | undefined, id: number): string {
  return catalog?.champions.find(c => c.id === id)?.name ?? `Campeão ${id}`;
}

/** Texto para comparar na busca: minúsculo, sem acento, sem espaço nem pontuação. */
export function searchKey(text: string): string {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');
}

/** Campeões cujo nome (ou nome interno em inglês) contém a busca. */
export function searchChampions(champions: Champion[], query: string): Champion[] {
  const key = searchKey(query);
  if (!key) return champions;
  return champions.filter(c => searchKey(c.name).includes(key) || searchKey(c.alias).includes(key));
}

/** Milissegundos que faltam para a etapa acabar, nunca negativo. */
export function remainingMs(champSelect: ChampSelect, now: number): number {
  return Math.max(0, champSelect.timer.endsAtMs - now);
}

/** Nome do jogador na seleção, ou um nome pela posição quando a LCU esconde. */
export function playerName(player: ChampSelectPlayer, index: number, ally: boolean): string {
  if (player.gameName) return player.gameName + (player.tagLine ? '#' + player.tagLine : '');
  return `${ally ? 'Aliado' : 'Adversário'} ${index + 1}`;
}

/** O que a grade mostra agora: a ação da vez do jogador decide entre ban e pick. */
export type GridPurpose = 'ban' | 'pick';

export function gridPurpose(champSelect: ChampSelect): GridPurpose {
  return champSelect.pendingAction?.type === 'ban' ? 'ban' : 'pick';
}

export type GridChampion = { champion: Champion; available: boolean };

/**
 * Campeões da grade: os que o jogador pode banir (na vez do ban) ou
 * escolher, conforme a LCU. Enquanto a LCU não mandou a lista, todos os do
 * catálogo. Banidos e já escolhidos por outro jogador ficam indisponíveis.
 */
export function gridChampions(state: Pick<AgentState, 'championCatalog' | 'champSelectChampions'>, champSelect: ChampSelect): GridChampion[] {
  const catalog = state.championCatalog?.champions ?? [];
  const purpose = gridPurpose(champSelect);
  const allowed = purpose === 'ban' ? state.champSelectChampions.bannable : state.champSelectChampions.pickable;
  const allowedSet = allowed ? new Set(allowed) : null;
  const taken = new Set<number>([...champSelect.bans.myTeam, ...champSelect.bans.theirTeam]);
  for (const action of champSelect.actions) {
    if (action.completed && action.championId && action.actorCellId !== champSelect.localPlayerCellId) taken.add(action.championId);
  }
  return catalog
    .filter(c => !allowedSet || allowedSet.has(c.id))
    .map(champion => ({ champion, available: !taken.has(champion.id) }));
}

/** `cellId`s de quem está na vez agora (ban ou pick em andamento). */
export function actingCells(champSelect: ChampSelect): Set<number> {
  return new Set(champSelect.actions.filter(a => a.isInProgress && !a.completed).map(a => a.actorCellId));
}

/** A ação do jogador logado que a grade usa agora. */
export type MyAction = {
  action: ChampSelectAction;
  /** É a vez dela: dá para marcar e confirmar. Fora da vez, só marcar o pick (intenção). */
  turn: boolean;
};

export function myAction(champSelect: ChampSelect): MyAction | null {
  if (champSelect.pendingAction) return { action: champSelect.pendingAction, turn: true };
  const pick = champSelect.actions.find(a => a.type === 'pick' && !a.completed && a.actorCellId === champSelect.localPlayerCellId);
  return pick ? { action: pick, turn: false } : null;
}

/** Campeão que a LCU já tem marcado para a ação do jogador (hover ou intenção). */
export function markedChampion(champSelect: ChampSelect, mine: MyAction): number | null {
  if (mine.turn) return mine.action.championId || null;
  const me = champSelect.myTeam.find(p => p.cellId === champSelect.localPlayerCellId);
  return me?.championPickIntent || null;
}

/** `queueId` do ARAM e da ARAM: Desordem, para quando a sessão não liga o banco. */
const aramQueueIds = new Set([450, 2400]);

/**
 * Seleção no estilo do ARAM (#201): sem ban nem pick, com campeão sorteado,
 * reroll e banco. Vale pelo banco ligado na sessão ou pela fila.
 */
export function isAramChampSelect(champSelect: ChampSelect): boolean {
  return champSelect.benchEnabled || champSelect.queue === 'aram' || aramQueueIds.has(champSelect.queueId);
}

import type { ReactNode } from 'react';
import type { AgentState } from '../bindings/AgentState';
import type { GameResult } from '../bindings/GameResult';
import type { GameResultTeam } from '../bindings/GameResultTeam';
import type { MatchPlayer } from '../bindings/MatchPlayer';
import { assetUrl } from '../agent/assets';
import { compactNumber, localPlayer, perMinute, rankedChangeText, resultQueue, resultTitle } from '../agent/gameResult';
import { formatDuration, kda, rankLabel } from '../agent/profile';
import { Avatar } from './base';
import { cx } from './base/cx';
import { GameIcon, ItemRow } from './GameIcons';

type Props = {
  result: GameResult;
  state: AgentState;
  /** Botões do pós-jogo (voltar ao lobby), no canto do cabeçalho. */
  actions?: ReactNode;
  /** Abre o perfil de um jogador pelo nick (#585). */
  onOpenProfile?: (puuid: string) => void;
};

/**
 * Resultado da partida que acabou de terminar (#183): vitória ou derrota,
 * fila, duração e PDL na ranqueada; o resumo do jogador logado; e o placar
 * dos times, com o time do jogador primeiro.
 */
export function GameResultView({ result, state, actions, onOpenProfile }: Props) {
  const me = localPlayer(result);
  return (
    <div className="@container flex w-full flex-col gap-4">
      <Header result={result} state={state} actions={actions} />
      {me && <Summary player={me} state={state} durationSecs={result.durationSecs} />}
      <div className="flex flex-col gap-3">
        {result.teams.map(team => (
          <TeamTable key={team.teamId} team={team} state={state} remake={result.result === 'remake'}
            localPuuid={result.localPuuid} durationSecs={result.durationSecs} onOpenProfile={onOpenProfile} />
        ))}
      </div>
    </div>
  );
}

function Header({ result, state, actions }: Props) {
  const change = result.rankedChange;
  const won = result.result === 'win';
  return (
    <header className="flex flex-wrap items-center justify-between gap-4">
      <div className="min-w-0">
        <h2 className={cx('text-title font-semibold',
          won ? 'text-accent-ink' : result.result === 'loss' ? 'text-danger' : 'text-muted')}>
          {resultTitle[result.result]}
        </h2>
        <p className="text-body text-muted">
          {resultQueue(result, state.queues)} · {formatDuration(result.durationSecs)}
          {result.result === 'remake' && ' · a partida não conta vitória nem derrota'}
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        {change && (
          <p aria-label="Mudança de elo" className={cx('rounded-control px-3 py-1.5 text-body font-semibold',
            won ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-danger')}>
            {rankedChangeText(change, won)}
            <span className="font-normal text-muted">
              {' · '}{rankLabel(change.after)}{change.after.tier && ` · ${change.after.leaguePoints} PDL`}
            </span>
          </p>
        )}
        {actions}
      </div>
    </header>
  );
}

function championName(state: AgentState, championId: number): string {
  return state.championCatalog?.champions.find(c => c.id === championId)?.name ?? `Campeão ${championId}`;
}

function Summary({ player: p, state, durationSecs }: { player: MatchPlayer; state: AgentState; durationSecs: number }) {
  const champion = championName(state, p.championId);
  const stats: [string, string][] = [
    ['KDA', `${p.kills} / ${p.deaths} / ${p.assists}`],
    ['Razão', kda(p.kills, p.deaths, p.assists)],
    ['CS', `${p.cs} (${perMinute(p.cs, durationSecs)}/min)`],
    ['Ouro', p.gold.toLocaleString('pt-BR')],
    ['Dano a campeões', p.damageToChampions.toLocaleString('pt-BR')],
    ['Placar de visão', String(p.visionScore)],
  ];
  return (
    <section aria-label="Seu desempenho" className="flex flex-wrap items-center gap-4 rounded-card bg-raised p-3">
      <div className="flex items-center gap-3">
        <span className="relative">
          <Avatar name={champion} src={assetUrl('champion', p.championId)} size={56} shape="rounded" />
          <span className="absolute -bottom-1 -right-1 rounded-full bg-surface px-1.5 text-caption font-semibold text-ink shadow-soft">
            {p.championLevel}
          </span>
        </span>
        <div className="min-w-0">
          <p className="text-body font-semibold text-ink">{champion}</p>
          <ItemRow items={p.items} size={22} />
        </div>
      </div>
      <dl className="grid min-w-[20rem] flex-1 grid-cols-3 gap-2 @min-[56rem]:grid-cols-6">
        {stats.map(([label, value]) => (
          <div key={label} className="min-w-0">
            <dt className="truncate text-caption text-muted">{label}</dt>
            <dd className="truncate text-body font-semibold text-ink">{value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function TeamTable({ team, state, remake, localPuuid, durationSecs, onOpenProfile }: {
  team: GameResultTeam; state: AgentState; remake: boolean; localPuuid: string; durationSecs: number;
  onOpenProfile?: (puuid: string) => void;
}) {
  const name = team.playerTeam ? 'Seu time' : 'Time inimigo';
  const outcome = remake ? '' : team.win ? ' · Vitória' : ' · Derrota';
  const kills = team.players.reduce((sum, p) => sum + p.kills, 0);
  const deaths = team.players.reduce((sum, p) => sum + p.deaths, 0);
  const assists = team.players.reduce((sum, p) => sum + p.assists, 0);
  const gold = team.players.reduce((sum, p) => sum + p.gold, 0);
  return (
    <table className="w-full table-fixed rounded-card bg-raised text-caption">
      <caption className="px-3 pb-1 pt-2 text-left">
        <span className={cx('text-body font-semibold', remake ? 'text-ink' : team.win ? 'text-accent-ink' : 'text-danger')}>
          {name}{outcome}
        </span>
        <span className="text-muted"> · {kills}/{deaths}/{assists} · {compactNumber(gold)} de ouro</span>
      </caption>
      <colgroup>
        <col />
        <col className="w-20" />
        <col className="w-12" />
        <col className="w-16 @max-[52rem]:hidden" />
        <col className="w-16 @max-[36rem]:hidden" />
        <col className="w-12 @max-[52rem]:hidden" />
        <col className="w-40" />
      </colgroup>
      <thead>
        <tr className="text-left text-muted">
          <th scope="col" className="px-3 py-1 font-semibold">Jogador</th>
          <th scope="col" className="px-1 py-1 font-semibold">K/D/A</th>
          <th scope="col" className="px-1 py-1 font-semibold">CS</th>
          <th scope="col" className="px-1 py-1 font-semibold @max-[52rem]:hidden">Ouro</th>
          <th scope="col" className="px-1 py-1 font-semibold @max-[36rem]:hidden">Dano</th>
          <th scope="col" className="px-1 py-1 font-semibold @max-[52rem]:hidden" title="Placar de visão">Visão</th>
          <th scope="col" className="px-3 py-1 font-semibold">Itens</th>
        </tr>
      </thead>
      <tbody>
        {team.players.map(p => {
          const me = localPuuid !== '' && p.puuid === localPuuid;
          return <PlayerRow key={`${p.puuid}-${p.championId}`} player={p} state={state} me={me} durationSecs={durationSecs}
            onOpen={onOpenProfile && p.puuid !== '' && !me ? () => onOpenProfile(p.puuid) : undefined} />;
        })}
      </tbody>
    </table>
  );
}

/**
 * Linha do placar. Com `onOpen`, o nick abre o perfil do jogador (#585), como
 * no placar do histórico (#308); o próprio jogador e quem veio sem `puuid`
 * (perfil privado) não são clicáveis.
 */
function PlayerRow({ player: p, state, me, durationSecs, onOpen }: {
  player: MatchPlayer; state: AgentState; me: boolean; durationSecs: number; onOpen?: () => void;
}) {
  const champion = championName(state, p.championId);
  const riotId = p.tagLine ? `${p.gameName}#${p.tagLine}` : p.gameName;
  return (
    <tr aria-current={me ? 'true' : undefined} className={cx('border-t border-line', me && 'bg-accent-soft')}>
      <td className="px-3 py-1">
        <div className="flex min-w-0 items-center gap-2">
          <span className="relative shrink-0">
            <Avatar name={champion} src={assetUrl('champion', p.championId)} size={30} shape="rounded" />
            <span className="absolute -bottom-1 -right-1 rounded-full bg-surface px-1 text-[0.625rem] font-semibold leading-4 text-ink">
              {p.championLevel}
            </span>
          </span>
          <span aria-hidden="true" className="flex shrink-0 flex-col gap-0.5">
            <GameIcon kind="summonerSpell" id={p.spell1Id} size={14} />
            <GameIcon kind="summonerSpell" id={p.spell2Id} size={14} />
          </span>
          <span className="min-w-0">
            {onOpen
              ? <button type="button" onClick={onOpen} title={riotId} aria-label={`Ver perfil de ${riotId || champion}`}
                className="-mx-1 block max-w-full truncate rounded-control-sm px-1 text-left text-ink hover:bg-surface hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent">
                {p.gameName || champion}
              </button>
              : <span className={cx('block truncate', me ? 'font-semibold text-ink' : 'text-ink')} title={riotId}>
                {p.gameName || champion}
              </span>}
            <span className="block truncate text-muted">{champion}</span>
          </span>
        </div>
      </td>
      <td className="whitespace-nowrap px-1">
        <span className="font-semibold text-ink">{p.kills}/{p.deaths}/{p.assists}</span>
        <span className="block text-muted">{kda(p.kills, p.deaths, p.assists)}</span>
      </td>
      <td className="px-1 text-ink" title={`${perMinute(p.cs, durationSecs)} por minuto`}>{p.cs}</td>
      <td className="px-1 text-ink @max-[52rem]:hidden">{compactNumber(p.gold)}</td>
      <td className="px-1 text-ink @max-[36rem]:hidden">{p.damageToChampions.toLocaleString('pt-BR')}</td>
      <td className="px-1 text-ink @max-[52rem]:hidden">{p.visionScore}</td>
      <td className="px-3 py-1"><ItemRow items={p.items} size={18} /></td>
    </tr>
  );
}

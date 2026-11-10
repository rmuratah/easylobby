import { useEffect, useState } from 'react';
import type { AgentState } from '../bindings/AgentState';
import type { LiveGame } from '../bindings/LiveGame';
import type { LivePlayer } from '../bindings/LivePlayer';
import type { LiveTeam } from '../bindings/LiveTeam';
import type { RankedEntry } from '../bindings/RankedEntry';
import type { TierDivision } from '../bindings/TierDivision';
import { assetUrl } from '../agent/assets';
import { liveTags, tierLabel, tierShort, type LiveTag } from '../agent/liveGame';
import { positionNames } from '../agent/positions';
import { formatDuration, rankLabel, winRate } from '../agent/profile';
import { Avatar, Spinner } from './base';
import { cx } from './base/cx';
import { PositionIcon } from './PositionIcon';

type Props = {
  state: AgentState;
  game: LiveGame;
  /** Abre o perfil de um jogador clicado (#308). Sem ele, os nomes não são clicáveis. */
  onOpenProfile?: (puuid: string) => void;
};

/**
 * Partida ao vivo (#268): cabeçalho com o tempo, a fila e o mapa, e uma
 * tabela por equipe com campeão, feitiços, runas, Riot ID, elo, taxa de
 * vitória ranqueada, partidas recentes com o campeão, rota principal, elo da
 * temporada passada e etiquetas.
 */
export function LiveGamePanel({ state, game, onOpenProfile }: Props) {
  const clock = useClock(game.gameTimeSecs);
  const queueName = game.rankedQueue === 'flex' ? 'Flex' : 'Solo/Duo';
  return (
    <section aria-label="Partida ao vivo" className="flex flex-col gap-4">
      <header className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <span className="inline-flex items-center gap-2 rounded-control-sm bg-danger px-2 py-0.5 text-caption font-bold uppercase tracking-wider text-on-accent">
          <span aria-hidden="true" className="size-2 rounded-full bg-on-accent motion-safe:animate-pulse" />
          Ao vivo
        </span>
        <span className="text-section font-semibold tabular-nums text-ink" aria-label="Tempo de jogo">
          {clock === null ? 'Carregando…' : formatDuration(clock)}
        </span>
        <h2 className="min-w-0 text-title font-semibold tracking-[-0.015em] text-ink">
          {game.queueName || 'Partida'}
          {game.mapName && <span className="ml-3 text-section font-normal text-muted">{game.mapName}</span>}
        </h2>
      </header>
      {game.teams.map(team => (
        <TeamTable key={team.teamId} state={state} team={team} queueName={queueName} onOpenProfile={onOpenProfile} />
      ))}
      <p className="text-caption text-muted">
        Elo e taxa de vitória da Ranqueada {queueName} na temporada atual. Campeão, rota principal e etiquetas vêm das
        últimas partidas do histórico de cada jogador (até 20). As runas aparecem depois da tela de carregamento.
      </p>
    </section>
  );
}

/**
 * Segundos de jogo, contando sozinho a partir da última leitura do agente.
 * O agente só manda o tempo de novo quando o relógio do jogo se afasta
 * desta conta (pausa, reconexão). `null` enquanto o jogo carrega.
 */
function useClock(secs: number | null): number | null {
  const [tick, setTick] = useState({ secs, elapsed: 0 });
  if (tick.secs !== secs) setTick({ secs, elapsed: 0 });
  useEffect(() => {
    if (secs === null) return;
    const started = Date.now();
    const id = window.setInterval(() => {
      setTick(t => t.secs === secs ? { secs, elapsed: Math.floor((Date.now() - started) / 1000) } : t);
    }, 1000);
    return () => window.clearInterval(id);
  }, [secs]);
  return secs === null ? null : secs + (tick.secs === secs ? tick.elapsed : 0);
}

function TeamTable({ state, team, queueName, onOpenProfile }: {
  state: AgentState; team: LiveTeam; queueName: string; onOpenProfile?: (puuid: string) => void;
}) {
  const blue = team.teamId === 100;
  const name = blue ? 'Equipe azul' : 'Equipe vermelha';
  return (
    <div className="overflow-x-auto rounded-panel surface card shadow-soft">
      {/* Larguras fixas: as colunas das duas equipes ficam alinhadas. */}
      <table className="w-full min-w-[62rem] table-fixed border-collapse text-caption">
        <colgroup>
          <col className="w-[18rem]" /><col className="w-[8.5rem]" /><col className="w-[11rem]" /><col className="w-[7rem]" />
          <col className="w-[6.5rem]" /><col className="w-[8rem]" /><col />
        </colgroup>
        <caption className="px-4 pt-3 pb-2 text-left">
          <span className="flex flex-wrap items-center gap-x-3 gap-y-1">
            <span className={cx('text-section font-semibold', blue ? 'text-status-game' : 'text-danger')}>{name}</span>
            <span className="text-muted">
              Elo médio: <span className="font-semibold text-ink">{team.averageTier ? tierLabel(team.averageTier) : '—'}</span>
            </span>
          </span>
        </caption>
        <thead>
          <tr className="border-y border-line text-left text-muted">
            <th scope="col" className="px-4 py-2 font-semibold">Jogador</th>
            <th scope="col" className="px-2 py-2 font-semibold">Elo ({queueName})</th>
            <th scope="col" className="px-2 py-2 font-semibold">Vitórias ranqueada</th>
            <th scope="col" className="px-2 py-2 font-semibold">Com o campeão</th>
            <th scope="col" className="px-2 py-2 text-center font-semibold">Rota principal</th>
            <th scope="col" className="px-2 py-2 text-center font-semibold">Temporada passada</th>
            <th scope="col" className="px-4 py-2 font-semibold">Etiquetas</th>
          </tr>
        </thead>
        <tbody>
          {team.players.map((player, i) => (
            <PlayerRow key={`${player.puuid}-${player.championId}-${i}`} state={state} player={player} blue={blue}
              onOpen={onOpenProfile && player.puuid && !player.me ? () => onOpenProfile(player.puuid) : undefined} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function championName(state: AgentState, championId: number): string {
  return state.championCatalog?.champions.find(c => c.id === championId)?.name ?? `Campeão ${championId}`;
}

function PlayerRow({ state, player: p, blue, onOpen }: {
  state: AgentState; player: LivePlayer; blue: boolean; onOpen?: () => void;
}) {
  const champion = championName(state, p.championId);
  const noData = p.bot || p.hidden;
  const loading = !noData && !p.stats;
  const name = p.hidden ? 'Jogador anônimo' : p.bot ? `${champion} (bot)` : p.gameName || (loading ? 'Carregando…' : champion);
  const riotId = p.tagLine ? `${p.gameName}#${p.tagLine}` : p.gameName;
  const label = <>
    <span className={cx('truncate font-semibold text-ink', onOpen && 'group-hover:underline')}>{name}</span>
    {!noData && p.tagLine && <span className="truncate text-muted">#{p.tagLine}</span>}
  </>;
  const empty = loading ? <Spinner size={14} label={null} /> : <span className="text-muted">—</span>;
  const stats = p.stats;
  return (
    <tr aria-label={`${noData ? name : riotId || name}, ${champion}`}
      className={cx('border-b border-line last:border-b-0', p.me && 'bg-accent-soft')}>
      <td className={cx('border-l-4 px-4 py-2', blue ? 'border-status-game' : 'border-danger')}>
        <div className="flex min-w-0 items-center gap-2">
          <Avatar name={champion} src={assetUrl('champion', p.championId)} size={36} shape="rounded" />
          <div className="grid grid-cols-2 gap-0.5" aria-hidden="true">
            <Icon kind="summonerSpell" id={p.spell1Id} />
            <Icon kind="perk" id={p.runes?.keystoneId ?? 0} />
            <Icon kind="summonerSpell" id={p.spell2Id} />
            <Icon kind="perkStyle" id={p.runes?.subStyleId ?? 0} />
          </div>
          {onOpen
            ? <button type="button" onClick={onOpen} title={riotId} aria-label={`Ver perfil de ${riotId || name}`}
              className="group flex min-w-0 max-w-[14rem] items-baseline gap-1 rounded-control-sm px-1 text-left hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent">
              {label}
            </button>
            : <span title={noData ? undefined : riotId} className="flex min-w-0 max-w-[14rem] items-baseline gap-1 px-1">{label}</span>}
        </div>
      </td>
      <td className="px-2 py-2">{stats?.rank ? <CurrentRank entry={stats.rank} /> : noData || stats ? <span className="text-muted">—</span> : empty}</td>
      <td className="px-2 py-2">{stats?.rank ? <RankedWinRate entry={stats.rank} /> : noData || stats ? <span className="text-muted">—</span> : empty}</td>
      <td className="px-2 py-2">{stats
        ? stats.recentGames === 0
          ? <span className="text-muted">Sem histórico</span>
          : <span title={`Entre as últimas ${stats.recentGames} partidas`}>
            <span className="font-semibold text-ink">{winRate(stats.championWins, stats.championGames - stats.championWins) ?? 0}%</span>
            <span className="block text-muted">{plural(stats.championGames, 'partida', 'partidas')}</span>
          </span>
        : noData ? <span className="text-muted">—</span> : empty}</td>
      <td className="px-2 py-2 text-center">{stats
        ? stats.mainPosition === 'unselected' || stats.mainPosition === 'fill'
          ? <span className="text-muted">—</span>
          : <span title={positionNames[stats.mainPosition]} className="inline-flex text-ink">
            <PositionIcon position={stats.mainPosition} size={20} />
            <span className="sr-only">{positionNames[stats.mainPosition]}</span>
          </span>
        : noData ? <span className="text-muted">—</span> : empty}</td>
      <td className="px-2 py-2 text-center">{stats
        ? stats.previousSeason ? <TierChip tier={stats.previousSeason} /> : <span className="text-muted">—</span>
        : noData ? <span className="text-muted">—</span> : empty}</td>
      <td className="px-4 py-2">
        <div className="flex flex-wrap gap-1">{liveTags(p).map(tag => <Tag key={tag.label} tag={tag} />)}</div>
      </td>
    </tr>
  );
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

function CurrentRank({ entry }: { entry: RankedEntry }) {
  if (!entry.tier) return <span className="text-muted">Sem classificação</span>;
  return (
    <span className="inline-flex items-center gap-2 whitespace-nowrap" title={rankLabel(entry)}>
      <TierChip tier={{ tier: entry.tier, division: entry.division }} />
      <span className="font-semibold text-ink">{entry.leaguePoints} PDL</span>
    </span>
  );
}

function RankedWinRate({ entry }: { entry: RankedEntry }) {
  const games = entry.wins + entry.losses;
  const rate = winRate(entry.wins, entry.losses);
  return (
    <span className="flex w-36 flex-col gap-1" title={`${entry.wins}V ${entry.losses}D`}>
      <span className="whitespace-nowrap">
        <span className={cx('font-semibold', rate === null ? 'text-muted' : rate >= 50 ? 'text-accent-ink' : 'text-danger')}>{rate ?? 0}%</span>
        <span className="text-muted"> ({plural(games, 'partida', 'partidas')})</span>
      </span>
      <span aria-hidden="true" className="h-1.5 overflow-hidden rounded-full bg-raised">
        <span className={cx('block h-full rounded-full', rate !== null && rate >= 50 ? 'bg-accent' : 'bg-danger')}
          style={{ width: `${rate ?? 0}%` }} />
      </span>
    </span>
  );
}

function TierChip({ tier }: { tier: TierDivision }) {
  return (
    <span title={tierLabel(tier)}
      className="inline-flex min-w-9 justify-center rounded-control-sm border border-accent/40 bg-surface px-1.5 py-0.5 text-caption font-bold text-accent-ink">
      <span aria-hidden="true">{tierShort(tier.tier, tier.division)}</span>
      <span className="sr-only">{tierLabel(tier)}</span>
    </span>
  );
}

function Tag({ tag }: { tag: LiveTag }) {
  return (
    <span title={tag.title}
      className={cx('whitespace-nowrap rounded-control-sm px-1.5 py-0.5 text-caption font-semibold',
        tag.tone === 'good' ? 'bg-accent-soft text-accent-ink' : tag.tone === 'bad' ? 'bg-danger text-on-accent' : 'bg-raised text-ink')}>
      {tag.label}
    </span>
  );
}

/** Ícone pequeno (feitiço, runa); espaço vazio ou imagem que não carregou viram um quadrado neutro. */
function Icon({ kind, id, size = 18 }: { kind: 'summonerSpell' | 'perk' | 'perkStyle'; id: number; size?: number }) {
  const [failed, setFailed] = useState<string | null>(null);
  const src = id ? assetUrl(kind, id) : null;
  if (!src || failed === src) return <span className="shrink-0 rounded-control-sm bg-raised" style={{ width: size, height: size }} />;
  return <img src={src} alt="" width={size} height={size} loading="lazy" onError={() => setFailed(src)}
    className="shrink-0 rounded-control-sm bg-raised" />;
}

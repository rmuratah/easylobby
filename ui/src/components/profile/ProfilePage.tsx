import { useId, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../../agent/client';
import type { AgentState } from '../../bindings/AgentState';
import type { RankedEntry } from '../../bindings/RankedEntry';
import type { MatchSummary } from '../../bindings/MatchSummary';
import type { MatchDetails } from '../../bindings/MatchDetails';
import type { MatchPlayer } from '../../bindings/MatchPlayer';
import { assetUrl } from '../../agent/assets';
import { useCommand } from '../../agent/useCommand';
import {
  formatDuration, kda, matchesFilter, matchFilters, matchStats, queueLabel, rankLabel, tierEmblemId, tierName, timeAgo, winRate,
  type MatchFilter,
} from '../../agent/profile';
import { Avatar, Button, EmptyState, Segmented, Spinner } from '../base';
import { cx } from '../base/cx';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type Tab = 'overview' | 'history';

/**
 * Perfil do jogador logado (#173): cabeçalho fixo e as abas Visão geral
 * (ranks, maestria e resumo) e Histórico (partidas, filtro e placar).
 */
export function ProfilePage({ connection, send }: Props) {
  const state = connection.status === 'connected' && connection.state?.lcu.state === 'connected' ? connection.state : null;
  const [tab, setTab] = useState<Tab>('overview');
  const id = useId();
  if (!state?.summoner) {
    return <EmptyState title="Perfil indisponível" description="Abra o LoL e entre na sua conta para ver o perfil." />;
  }
  const summoner = state.summoner;
  const name = summoner.gameName || summoner.displayName || 'Invocador';
  const tabs: { value: Tab; label: string }[] = [
    { value: 'overview', label: 'Visão geral' },
    { value: 'history', label: 'Histórico' },
  ];
  return (
    <div className="flex flex-col gap-5">
      <header className="flex items-center gap-4 rounded-panel surface p-5 shadow-soft animate-fade-up">
        <Avatar name={name} src={assetUrl('profileIcon', summoner.profileIconId)} size={72} />
        <div className="min-w-0">
          <h2 className="break-words text-title font-semibold text-ink">
            {name}{summoner.gameName && summoner.tagLine && <span className="font-medium text-muted">#{summoner.tagLine}</span>}
          </h2>
          <p className="mt-1 w-fit rounded-full bg-raised px-3 py-1 text-caption font-semibold text-muted">Nível {summoner.summonerLevel}</p>
        </div>
      </header>
      <div role="tablist" aria-label="Seções do perfil" className="flex gap-1 border-b border-line">
        {tabs.map(t => (
          <button key={t.value} type="button" role="tab" id={`${id}-${t.value}-tab`} aria-controls={`${id}-${t.value}`}
            aria-selected={tab === t.value} tabIndex={tab === t.value ? 0 : -1}
            onClick={() => setTab(t.value)}
            onKeyDown={event => {
              if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
              const next = tab === 'overview' ? 'history' : 'overview';
              setTab(next);
              document.getElementById(`${id}-${next}-tab`)?.focus();
            }}
            className={cx('-mb-px border-b-2 px-4 py-2 text-body font-semibold',
              'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
              tab === t.value ? 'border-accent text-accent-ink' : 'border-transparent text-muted hover:text-ink')}>
            {t.label}
          </button>
        ))}
      </div>
      <div role="tabpanel" id={`${id}-overview`} aria-labelledby={`${id}-overview-tab`} hidden={tab !== 'overview'}>
        <Overview state={state} />
      </div>
      <div role="tabpanel" id={`${id}-history`} aria-labelledby={`${id}-history-tab`} hidden={tab !== 'history'}>
        <History state={state} send={send} />
      </div>
    </div>
  );
}

function championName(state: AgentState, championId: number): string {
  return state.championCatalog?.champions.find(c => c.id === championId)?.name ?? `Campeão ${championId}`;
}

function Overview({ state }: { state: AgentState }) {
  const matches = state.matchHistory?.matches ?? [];
  const stats = matchStats(matches);
  return (
    <div className="grid gap-4 @min-[44rem]:grid-cols-2">
      <section aria-label="Ranks" className="grid gap-4 @min-[44rem]:col-span-2 @min-[44rem]:grid-cols-2">
        <RankCard title="Ranqueada Solo/Duo" entry={state.ranked?.soloDuo ?? null} />
        <RankCard title="Ranqueada Flex" entry={state.ranked?.flex ?? null} />
      </section>
      <section aria-labelledby="perfil-maestria" className="rounded-panel surface p-5 shadow-soft">
        <h3 id="perfil-maestria" className="text-section font-semibold text-ink">Maestria</h3>
        {state.championMasteries.length === 0
          ? <p className="mt-3 text-body text-muted">Nenhuma maestria para mostrar.</p>
          : <ol className="mt-3 flex flex-col gap-3">
            {state.championMasteries.map((m, i) => {
              const champion = championName(state, m.championId);
              return (
                <li key={m.championId} className="flex items-center gap-3">
                  <span className="w-4 text-caption font-semibold text-muted">{i + 1}</span>
                  <Avatar name={champion} src={assetUrl('champion', m.championId)} size={40} shape="rounded" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-body font-semibold text-ink">{champion}</p>
                    <p className="text-caption text-muted">Nível {m.level} · {m.points.toLocaleString('pt-BR')} pontos</p>
                  </div>
                </li>
              );
            })}
          </ol>}
      </section>
      <section aria-labelledby="perfil-resumo" className="rounded-panel surface p-5 shadow-soft">
        <h3 id="perfil-resumo" className="text-section font-semibold text-ink">Últimas {stats.games} partidas</h3>
        {stats.games === 0
          ? <p className="mt-3 text-body text-muted">{state.matchHistory ? 'Nenhuma partida no histórico.' : 'Carregando o histórico…'}</p>
          : <>
            <dl className="mt-3 grid grid-cols-3 gap-2">
              <Stat label="Vitórias" value={`${stats.wins}V ${stats.losses}D`} />
              <Stat label="Taxa de vitória" value={`${stats.winRate ?? 0}%`} />
              <Stat label="KDA médio" value={`${fmt(stats.kills)} / ${fmt(stats.deaths)} / ${fmt(stats.assists)}`} />
            </dl>
            <h4 className="mt-4 text-caption font-semibold text-muted">Mais jogados</h4>
            <ul className="mt-2 flex flex-col gap-2">
              {stats.champions.map(c => {
                const champion = championName(state, c.championId);
                return (
                  <li key={c.championId} className="flex items-center gap-3">
                    <Avatar name={champion} src={assetUrl('champion', c.championId)} size={32} shape="rounded" />
                    <span className="min-w-0 flex-1 truncate text-body text-ink">{champion}</span>
                    <span className="text-caption text-muted">{c.games} {c.games === 1 ? 'partida' : 'partidas'} · {winRate(c.wins, c.games - c.wins)}%</span>
                  </li>
                );
              })}
            </ul>
          </>}
      </section>
    </div>
  );
}

const fmt = (n: number) => n.toFixed(1).replace('.', ',');

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-tile bg-raised p-3">
      <dt className="text-caption font-semibold text-muted">{label}</dt>
      <dd className="mt-1 text-body font-semibold text-ink">{value}</dd>
    </div>
  );
}

function RankCard({ title, entry }: { title: string; entry: RankedEntry | null }) {
  const rate = entry ? winRate(entry.wins, entry.losses) : null;
  return (
    <article aria-label={title} className="flex items-center gap-4 rounded-panel surface p-5 shadow-soft">
      <TierEmblem tier={entry?.tier ?? null} />
      <div className="min-w-0">
        <h3 className="text-caption font-semibold text-muted">{title}</h3>
        {!entry ? <p className="mt-1 text-body text-muted">Carregando…</p> : <>
          <p className="mt-1 text-section font-semibold text-ink">{rankLabel(entry)}{entry.tier && <span className="text-muted"> · {entry.leaguePoints} PDL</span>}</p>
          <p className="text-caption text-muted">
            {entry.wins + entry.losses === 0 ? 'Nenhuma partida nesta temporada'
              : `${entry.wins}V ${entry.losses}D · ${rate}% de vitórias`}
            {entry.provisional && ' · em classificação'}
          </p>
        </>}
      </div>
    </article>
  );
}

/** Emblema do elo; sem classificação, ou se a imagem não carregar, um círculo com a sigla do tier. */
function TierEmblem({ tier }: { tier: string | null }) {
  const [failed, setFailed] = useState<string | null>(null);
  const id = tier ? tierEmblemId(tier) : null;
  const src = id ? assetUrl('rankedEmblem', id) : null;
  if (tier && src && failed !== src) {
    return <img src={src} alt="" width={64} height={64} onError={() => setFailed(src)}
      className="size-16 shrink-0 object-contain" />;
  }
  return (
    <span aria-hidden="true"
      className={cx('flex size-16 shrink-0 items-center justify-center rounded-full border-2 text-caption font-bold uppercase',
        tier ? 'border-accent bg-accent-soft text-accent-ink' : 'border-line bg-raised text-muted')}>
      {tier ? tierName(tier).slice(0, 3) : '—'}
    </span>
  );
}

function History({ state, send }: { state: AgentState; send: AgentClient['send'] }) {
  const [filter, setFilter] = useState<MatchFilter>('all');
  const [open, setOpen] = useState<number | null>(null);
  const [now] = useState(() => Date.now());
  const more = useCommand(send);
  const details = useCommand(send);
  const history = state.matchHistory;
  if (!history) return <p className="py-6 text-body text-muted">Carregando o histórico…</p>;
  const shown = history.matches.filter(m => matchesFilter(m, filter));
  function toggle(match: MatchSummary) {
    if (open === match.gameId) {
      setOpen(null);
      return;
    }
    setOpen(match.gameId);
    if (state.matchDetails?.gameId !== match.gameId) {
      void details.run({ type: 'openMatchDetails', gameId: match.gameId }, { pending: 'Carregando o placar…', failure: 'carregar o placar' });
    }
  }
  return (
    <div className="flex flex-col gap-4">
      <Segmented label="Filtrar por fila" value={filter} onChange={setFilter} className="w-fit max-w-full flex-wrap"
        options={matchFilters.map(f => ({ value: f.value, label: f.label }))} />
      {shown.length === 0
        ? <p className="py-4 text-body text-muted">
          {history.matches.length === 0 ? 'Nenhuma partida no histórico.' : `Nenhuma partida desta fila entre as ${history.matches.length} carregadas.`}
        </p>
        : <ul aria-label="Partidas" className="flex flex-col gap-2">
          {shown.map(match => (
            <li key={match.gameId}>
              <MatchRow state={state} match={match} now={now} open={open === match.gameId} onToggle={() => toggle(match)} />
              {open === match.gameId && (
                <div id={`placar-${match.gameId}`} className="mt-2">
                  {state.matchDetails?.gameId === match.gameId
                    ? <Scoreboard state={state} details={state.matchDetails} />
                    : details.feedback.status === 'error'
                      ? <p role="alert" className="text-body text-danger">{details.feedback.message}</p>
                      : <p className="flex items-center gap-2 text-body text-muted"><Spinner size={18} label={null} /> Carregando o placar…</p>}
                </div>
              )}
            </li>
          ))}
        </ul>}
      {history.hasMore && (
        <Button className="self-center" loading={more.pending} disabled={more.pending}
          onClick={() => void more.run({ type: 'loadMoreMatches' }, { pending: 'Carregando…', failure: 'carregar mais partidas' })}>
          Carregar mais
        </Button>
      )}
      {more.feedback.status === 'error' && <p role="alert" className="text-center text-body text-danger">{more.feedback.message}</p>}
    </div>
  );
}

const resultText = { win: 'Vitória', loss: 'Derrota', remake: 'Remake' } as const;

function MatchRow({ state, match, now, open, onToggle }: {
  state: AgentState; match: MatchSummary; now: number; open: boolean; onToggle: () => void;
}) {
  const p = match.player;
  const champion = championName(state, p.championId);
  return (
    <button type="button" aria-expanded={open} aria-controls={`placar-${match.gameId}`} onClick={onToggle}
      aria-label={`${resultText[match.result]} com ${champion}, ${queueLabel(match.queueId, state.queues)}, ${p.kills}/${p.deaths}/${p.assists}`}
      className={cx('flex w-full flex-wrap items-center gap-x-4 gap-y-2 rounded-card border-l-4 bg-surface p-3 text-left shadow-soft',
        'hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
        match.result === 'win' ? 'border-accent' : match.result === 'loss' ? 'border-danger' : 'border-line')}>
      <div className="w-28 shrink-0">
        <p className={cx('text-body font-semibold',
          match.result === 'win' ? 'text-accent-ink' : match.result === 'loss' ? 'text-danger' : 'text-muted')}>{resultText[match.result]}</p>
        <p className="truncate text-caption text-muted">{queueLabel(match.queueId, state.queues)}</p>
        <p className="text-caption text-muted">{formatDuration(match.durationSecs)} · {timeAgo(match.createdAt, now)}</p>
      </div>
      <div className="flex items-center gap-2">
        <Avatar name={champion} src={assetUrl('champion', p.championId)} size={44} shape="rounded" />
        <div className="grid grid-cols-2 gap-0.5" aria-hidden="true">
          <Icon kind="summonerSpell" id={p.spell1Id} />
          <Icon kind="perk" id={p.perkId} />
          <Icon kind="summonerSpell" id={p.spell2Id} />
          <Icon kind="perkStyle" id={p.perkSubStyleId} />
        </div>
      </div>
      <div className="w-24 shrink-0">
        <p className="text-body font-semibold text-ink">{p.kills} / {p.deaths} / {p.assists}</p>
        <p className="text-caption text-muted">KDA {kda(p.kills, p.deaths, p.assists)} · {p.cs} CS</p>
      </div>
      <Items items={p.items} />
    </button>
  );
}

/** Ícone pequeno (feitiço, runa, item); espaço vazio ou imagem que não carregou viram um quadrado neutro. */
function Icon({ kind, id, size = 20 }: { kind: 'summonerSpell' | 'perk' | 'perkStyle' | 'item'; id: number; size?: number }) {
  const [failed, setFailed] = useState<string | null>(null);
  const src = id ? assetUrl(kind, id) : null;
  if (!src || failed === src) return <span className="shrink-0 rounded-control-sm bg-raised" style={{ width: size, height: size }} />;
  return <img src={src} alt="" width={size} height={size} loading="lazy" onError={() => setFailed(src)}
    className="shrink-0 rounded-control-sm bg-raised" />;
}

function Items({ items, size = 24 }: { items: number[]; size?: number }) {
  return (
    <div aria-hidden="true" className="flex gap-0.5">
      {items.map((item, i) => <Icon key={i} kind="item" id={item} size={size} />)}
    </div>
  );
}

function Scoreboard({ state, details }: { state: AgentState; details: MatchDetails }) {
  const teams = [100, 200].map(teamId => details.players.filter(p => p.teamId === teamId));
  const me = state.summoner?.puuid;
  return (
    <div className="grid gap-3 rounded-card bg-raised p-3 @min-[56rem]:grid-cols-2">
      {teams.map((players, i) => {
        const won = players.some(p => p.win);
        const label = `${i === 0 ? 'Time azul' : 'Time vermelho'}${details.remake ? '' : won ? ' · Vitória' : ' · Derrota'}`;
        return (
          <table key={i} className="w-full text-caption">
            <caption className={cx('pb-1 text-left text-body font-semibold', won && !details.remake ? 'text-accent-ink' : 'text-danger')}>{label}</caption>
            <thead className="sr-only">
              <tr><th>Jogador</th><th>K/D/A</th><th>CS</th><th>Dano</th><th>Itens</th></tr>
            </thead>
            <tbody>
              {players.map(p => <ScoreRow key={`${p.teamId}-${p.puuid}-${p.championId}`} state={state} player={p} me={p.puuid !== '' && p.puuid === me} />)}
            </tbody>
          </table>
        );
      })}
    </div>
  );
}

function ScoreRow({ state, player: p, me }: { state: AgentState; player: MatchPlayer; me: boolean }) {
  const champion = championName(state, p.championId);
  return (
    <tr className={cx(me && 'bg-accent-soft')}>
      <td className="py-1 pr-2">
        <div className="flex min-w-0 items-center gap-2">
          <Avatar name={champion} src={assetUrl('champion', p.championId)} size={28} shape="rounded" />
          <span className="truncate text-ink" title={p.tagLine ? `${p.gameName}#${p.tagLine}` : p.gameName}>
            {p.gameName || champion}
          </span>
        </div>
      </td>
      <td className="whitespace-nowrap px-1 font-semibold text-ink">{p.kills}/{p.deaths}/{p.assists}</td>
      <td className="px-1 text-muted">{p.cs}</td>
      <td className="px-1 text-muted">{p.damageToChampions.toLocaleString('pt-BR')}</td>
      <td className="py-1 pl-1"><Items items={p.items} size={18} /></td>
    </tr>
  );
}

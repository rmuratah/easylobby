import { useEffect, useId, useState, type ReactNode } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../../agent/client';
import type { AgentState } from '../../bindings/AgentState';
import type { RankedEntry } from '../../bindings/RankedEntry';
import type { MatchSummary } from '../../bindings/MatchSummary';
import type { MatchDetails } from '../../bindings/MatchDetails';
import type { MatchPlayer } from '../../bindings/MatchPlayer';
import type { MatchHistory } from '../../bindings/MatchHistory';
import type { RankedStats } from '../../bindings/RankedStats';
import type { ChampionMastery } from '../../bindings/ChampionMastery';
import type { Summoner } from '../../bindings/Summoner';
import type { Command } from '../../bindings/Command';
import { availabilityColors, describeActivity } from '../../agent/friends';
import { assetUrl } from '../../agent/assets';
import { useCommand } from '../../agent/useCommand';
import {
  formatDuration, kda, matchesFilter, matchFilters, matchStats, queueLabel, rankLabel, tierEmblemId, tierName, timeAgo, winRate,
  type MatchFilter,
} from '../../agent/profile';
import { Avatar, Button, EmptyState, Segmented, Spinner } from '../base';
import { cx } from '../base/cx';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /** Abre o perfil de outro jogador clicado no placar (#308). Sem ele, os nomes não são clicáveis. */
  onOpenProfile?: (puuid: string) => void;
};
type Tab = 'overview' | 'history';

/** O que a página mostra de um jogador: o logado ou outro (#255). */
type ProfileData = {
  summoner: Summoner;
  ranked: RankedStats | null;
  championMasteries: ChampionMastery[];
  matchHistory: MatchHistory | null;
  /** Perfil de outro jogador já carregado: o que falta não vai mais chegar. */
  loaded: boolean;
  privateProfile: boolean;
  /** Comando do "Carregar mais" do histórico. */
  loadMore: Command;
};

function connectedState(connection: ConnectionSnapshot): AgentState | null {
  return connection.status === 'connected' && connection.state?.lcu.state === 'connected' ? connection.state : null;
}

/**
 * Perfil do jogador logado (#173): cabeçalho fixo e as abas Visão geral
 * (ranks, maestria e resumo) e Histórico (partidas, filtro e placar).
 */
export function ProfilePage({ connection, send, onOpenProfile }: Props) {
  const state = connectedState(connection);
  if (!state?.summoner) {
    return <EmptyState title="Perfil indisponível" description="Abra o LoL e entre na sua conta para ver o perfil." />;
  }
  const data: ProfileData = {
    summoner: state.summoner, ranked: state.ranked, championMasteries: state.championMasteries,
    matchHistory: state.matchHistory, loaded: false, privateProfile: false, loadMore: { type: 'loadMoreMatches' },
  };
  return <ProfileView state={state} data={data} send={send} onOpenProfile={onOpenProfile} />;
}

/** Intervalo de atualização do "há N min" do status do amigo. */
const CLOCK_INTERVAL_MS = 30_000;

/**
 * Perfil de um amigo (#255), no mesmo formato do perfil do jogador logado.
 * Pede o perfil ao abrir e de novo a cada amigo novo; o cabeçalho mostra o
 * status dele e o botão de conversar.
 */
export function FriendProfilePage({ connection, send, puuid, onBack, onChat, onOpenProfile }: Props & {
  puuid: string; onBack: () => void; onChat: (conversationId: string) => void;
}) {
  const state = connectedState(connection);
  const load = useCommand(send);
  const { run } = load;
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), CLOCK_INTERVAL_MS);
    return () => clearInterval(timer);
  }, []);
  const connected = state !== null;
  useEffect(() => {
    if (!connected) return;
    void run({ type: 'openPlayerProfile', puuid }, { pending: 'Carregando o perfil…', failure: 'carregar o perfil' });
    // `run` muda a cada render; o perfil só é pedido de novo para outro amigo ou depois de uma queda.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [puuid, connected]);
  const back = <button type="button" className="nav-button self-start" onClick={onBack}>Voltar</button>;
  if (!state) {
    return <div className="flex flex-col gap-4">{back}<EmptyState title="Perfil indisponível" description="Abra o LoL e entre na sua conta para ver o perfil do amigo." /></div>;
  }
  const friend = state.friends.find(f => f.puuid === puuid) ?? null;
  const profile = state.playerProfile?.summoner.puuid === puuid ? state.playerProfile : null;
  if (!profile) {
    return (
      <div className="flex flex-col gap-4">
        {back}
        {load.feedback.status === 'error'
          ? <EmptyState title="Perfil indisponível" description={load.feedback.message}
            action={<Button onClick={() => void run({ type: 'openPlayerProfile', puuid }, { pending: 'Carregando o perfil…', failure: 'carregar o perfil' })}>Tentar de novo</Button>} />
          : <p className="flex items-center gap-2 py-6 text-body text-muted"><Spinner size={18} label={null} /> Carregando o perfil…</p>}
      </div>
    );
  }
  const data: ProfileData = {
    summoner: profile.summoner, ranked: profile.ranked, championMasteries: profile.championMasteries,
    matchHistory: profile.matchHistory, loaded: true, privateProfile: profile.privateProfile, loadMore: { type: 'loadMorePlayerMatches' },
  };
  const extra = friend && (
    <div className="flex flex-wrap items-center gap-3 @min-[44rem]:ml-auto">
      <p className="flex items-center gap-2 text-caption text-muted">
        <span aria-hidden="true" className={cx('h-2.5 w-2.5 shrink-0 rounded-full', availabilityColors[friend.availability])} />
        {describeActivity(friend, now)}
      </p>
      <Button size="sm" onClick={() => onChat(friend.id)}>Conversar</Button>
    </div>
  );
  return (
    <div className="flex flex-col gap-4">
      {back}
      <ProfileView state={state} data={data} send={send} extra={extra} onOpenProfile={onOpenProfile} />
    </div>
  );
}

function ProfileView({ state, data, send, extra, onOpenProfile }: {
  state: AgentState; data: ProfileData; send: AgentClient['send']; extra?: ReactNode; onOpenProfile?: (puuid: string) => void;
}) {
  const [tab, setTab] = useState<Tab>('overview');
  const id = useId();
  const summoner = data.summoner;
  const name = summoner.gameName || summoner.displayName || 'Invocador';
  const tabs: { value: Tab; label: string }[] = [
    { value: 'overview', label: 'Visão geral' },
    { value: 'history', label: 'Histórico' },
  ];
  return (
    <div className="flex flex-col gap-5">
      <header className="flex flex-wrap items-center gap-4 rounded-panel surface p-5 shadow-soft animate-fade-up">
        <Avatar name={name} src={assetUrl('profileIcon', summoner.profileIconId)} size={72} />
        <div className="min-w-0">
          <h2 className="break-words text-title font-semibold text-ink">
            {name}{summoner.gameName && summoner.tagLine && <span className="font-medium text-muted">#{summoner.tagLine}</span>}
          </h2>
          <div className="mt-1 flex flex-wrap gap-2">
            <p className="w-fit rounded-full bg-raised px-3 py-1 text-caption font-semibold text-muted">Nível {summoner.summonerLevel}</p>
            {data.privateProfile && <p className="w-fit rounded-full bg-raised px-3 py-1 text-caption font-semibold text-muted">Perfil privado</p>}
          </div>
        </div>
        {extra}
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
        <Overview state={state} data={data} />
      </div>
      <div role="tabpanel" id={`${id}-history`} aria-labelledby={`${id}-history-tab`} hidden={tab !== 'history'}>
        <History state={state} data={data} send={send} onOpenProfile={onOpenProfile} />
      </div>
    </div>
  );
}

function championName(state: AgentState, championId: number): string {
  return state.championCatalog?.champions.find(c => c.id === championId)?.name ?? `Campeão ${championId}`;
}

function Overview({ state, data }: { state: AgentState; data: ProfileData }) {
  const matches = data.matchHistory?.matches ?? [];
  const missing = data.loaded ? 'Indisponível' : 'Carregando…';
  const stats = matchStats(matches);
  return (
    <div className="grid gap-4 @min-[44rem]:grid-cols-2">
      <section aria-label="Ranks" className="grid gap-4 @min-[44rem]:col-span-2 @min-[44rem]:grid-cols-2">
        <RankCard title="Ranqueada Solo/Duo" entry={data.ranked?.soloDuo ?? null} missing={missing} />
        <RankCard title="Ranqueada Flex" entry={data.ranked?.flex ?? null} missing={missing} />
      </section>
      <section aria-labelledby="perfil-maestria" className="rounded-panel surface p-5 shadow-soft">
        <h3 id="perfil-maestria" className="text-section font-semibold text-ink">Maestria</h3>
        {data.championMasteries.length === 0
          ? <p className="mt-3 text-body text-muted">Nenhuma maestria para mostrar.</p>
          : <ol className="mt-3 flex flex-col gap-3">
            {data.championMasteries.map((m, i) => {
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
          ? <p className="mt-3 text-body text-muted">{emptyHistory(data)}</p>
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

/** Texto do histórico sem partidas: carregando, privado, indisponível ou vazio. */
function emptyHistory(data: ProfileData): string {
  if (data.privateProfile) return 'O histórico deste jogador é privado.';
  if (!data.matchHistory) return data.loaded ? 'Histórico indisponível.' : 'Carregando o histórico…';
  return 'Nenhuma partida no histórico.';
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

function RankCard({ title, entry, missing }: { title: string; entry: RankedEntry | null; missing: string }) {
  const rate = entry ? winRate(entry.wins, entry.losses) : null;
  return (
    <article aria-label={title} className="flex items-center gap-4 rounded-panel surface p-5 shadow-soft">
      <TierEmblem tier={entry?.tier ?? null} />
      <div className="min-w-0">
        <h3 className="text-caption font-semibold text-muted">{title}</h3>
        {!entry ? <p className="mt-1 text-body text-muted">{missing}</p> : <>
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
    // A imagem do CommunityDragon tem muita borda transparente: ela fica com
    // o dobro do tamanho da caixa, centralizada, e a borda sobra para fora.
    return (
      <span aria-hidden="true" className="relative -my-3 size-28 shrink-0">
        <img src={src} alt="" width={224} height={224} onError={() => setFailed(src)}
          className="pointer-events-none absolute left-1/2 top-1/2 size-56 max-w-none -translate-x-1/2 -translate-y-1/2 object-contain" />
      </span>
    );
  }
  return (
    <span aria-hidden="true"
      className={cx('flex size-16 shrink-0 items-center justify-center rounded-full border-2 text-caption font-bold uppercase',
        tier ? 'border-accent bg-accent-soft text-accent-ink' : 'border-line bg-raised text-muted')}>
      {tier ? tierName(tier).slice(0, 3) : '—'}
    </span>
  );
}

function History({ state, data, send, onOpenProfile }: {
  state: AgentState; data: ProfileData; send: AgentClient['send']; onOpenProfile?: (puuid: string) => void;
}) {
  const [filter, setFilter] = useState<MatchFilter>('all');
  const [open, setOpen] = useState<number | null>(null);
  const [now] = useState(() => Date.now());
  const more = useCommand(send);
  const details = useCommand(send);
  const history = data.matchHistory;
  if (!history || (history.matches.length === 0 && data.privateProfile)) return <p className="py-6 text-body text-muted">{emptyHistory(data)}</p>;
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
                    ? <Scoreboard state={state} details={state.matchDetails} highlight={data.summoner.puuid} onOpenProfile={onOpenProfile} />
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
          onClick={() => void more.run(data.loadMore, { pending: 'Carregando…', failure: 'carregar mais partidas' })}>
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

/**
 * Placar com os dois times; a linha de `highlight` (o dono do perfil) fica
 * destacada. Com `onOpenProfile`, o nome dos outros jogadores abre o perfil
 * deles (#308); quem veio sem `puuid` (perfil privado) não é clicável.
 */
function Scoreboard({ state, details, highlight, onOpenProfile }: {
  state: AgentState; details: MatchDetails; highlight: string; onOpenProfile?: (puuid: string) => void;
}) {
  const teams = [100, 200].map(teamId => details.players.filter(p => p.teamId === teamId));
  const me = highlight;
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
              {players.map(p => {
                const isOwner = p.puuid !== '' && p.puuid === me;
                return <ScoreRow key={`${p.teamId}-${p.puuid}-${p.championId}`} state={state} player={p} me={isOwner}
                  onOpen={onOpenProfile && p.puuid !== '' && !isOwner ? () => onOpenProfile(p.puuid) : undefined} />;
              })}
            </tbody>
          </table>
        );
      })}
    </div>
  );
}

function ScoreRow({ state, player: p, me, onOpen }: { state: AgentState; player: MatchPlayer; me: boolean; onOpen?: () => void }) {
  const champion = championName(state, p.championId);
  const riotId = p.tagLine ? `${p.gameName}#${p.tagLine}` : p.gameName;
  const content = <>
    <Avatar name={champion} src={assetUrl('champion', p.championId)} size={28} shape="rounded" />
    <span className={cx('truncate text-ink', onOpen && 'group-hover:underline')} title={riotId}>
      {p.gameName || champion}
    </span>
  </>;
  return (
    <tr className={cx(me && 'bg-accent-soft')}>
      <td className="py-1 pr-2">
        {onOpen
          ? <button type="button" onClick={onOpen} aria-label={`Ver perfil de ${riotId || champion}`}
            className="group -mx-1 flex min-w-0 max-w-full items-center gap-2 rounded-control-sm px-1 text-left hover:bg-surface focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent">
            {content}
          </button>
          : <div className="flex min-w-0 items-center gap-2">{content}</div>}
      </td>
      <td className="whitespace-nowrap px-1 font-semibold text-ink">{p.kills}/{p.deaths}/{p.assists}</td>
      <td className="px-1 text-muted">{p.cs}</td>
      <td className="px-1 text-muted">{p.damageToChampions.toLocaleString('pt-BR')}</td>
      <td className="py-1 pl-1"><Items items={p.items} size={18} /></td>
    </tr>
  );
}

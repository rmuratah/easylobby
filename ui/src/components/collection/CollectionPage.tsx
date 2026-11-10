import { useEffect, useState, type ReactNode } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../../agent/client';
import type { AgentState } from '../../bindings/AgentState';
import type { CollectionChampion } from '../../bindings/CollectionChampion';
import type { CollectionSkin } from '../../bindings/CollectionSkin';
import { assetUrl } from '../../agent/assets';
import { useCommand } from '../../agent/useCommand';
import { LANES, positionNames } from '../../agent/positions';
import {
  CHAMPION_ROLES, emptyFilters, filterCollection, hasLanes, percent, purchasedLabel, rarityName, roleNames, skinTotals,
  type CollectionFilters, type OwnershipFilter,
} from '../../agent/collection';
import { Button, EmptyState, SearchField, Segmented, Spinner } from '../base';
import { cx } from '../base/cx';
import { ChampionIcon } from '../ChampSelectPanel';
import { PositionIcon } from '../PositionIcon';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

function connectedState(connection: ConnectionSnapshot): AgentState | null {
  return connection.status === 'connected' && connection.state?.lcu.state === 'connected' ? connection.state : null;
}

const ownershipOptions: { value: OwnershipFilter; label: string }[] = [
  { value: 'all', label: 'Todos' },
  { value: 'owned', label: 'Tenho' },
  { value: 'missing', label: 'Não tenho' },
];

/**
 * Aba Coleção (#369): a grade com todos os campeões, os da conta e os que
 * faltam, com busca e filtros por função e rota. Abrir um campeão mostra as
 * skins e os cromas dele. Só mostra; comprar fica de fora (decisão "sem loja").
 */
export function CollectionPage({ connection, send }: Props) {
  const state = connectedState(connection);
  const load = useCommand(send);
  const { run } = load;
  const [filters, setFilters] = useState<CollectionFilters>(emptyFilters);
  const [open, setOpen] = useState<number | null>(null);
  const connected = state?.summoner != null;
  const reload = () => void run({ type: 'openCollection' }, { pending: 'Carregando a coleção…', failure: 'carregar a coleção' });
  useEffect(() => {
    if (connected) reload();
    // `run` muda a cada render; a coleção é pedida ao abrir a aba e de novo depois de uma queda.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected]);
  if (!state?.summoner) {
    return <EmptyState title="Coleção indisponível" description="Abra o LoL e entre na sua conta para ver a sua coleção." />;
  }
  const collection = state.collection;
  if (!collection) {
    return load.feedback.status === 'error'
      ? <EmptyState title="Coleção indisponível" description={load.feedback.message} action={<Button onClick={reload}>Tentar de novo</Button>} />
      : <p className="flex items-center gap-2 py-6 text-body text-muted"><Spinner size={18} label={null} /> Carregando a coleção…</p>;
  }
  const champion = open === null ? null : collection.champions.find(c => c.id === open) ?? null;
  if (champion) {
    return <ChampionSkinsView state={state} champion={champion} send={send} onBack={() => setOpen(null)} />;
  }
  const shown = filterCollection(collection.champions, filters);
  const showLanes = hasLanes(collection.champions);
  const set = (change: Partial<CollectionFilters>) => setFilters(current => ({ ...current, ...change }));
  return (
    <div className="flex flex-col gap-5">
      <header className="flex flex-wrap items-end gap-x-8 gap-y-3 rounded-panel surface card p-5 shadow-soft animate-fade-up">
        <div className="mr-auto">
          <h2 className="text-title font-semibold text-ink">Coleção</h2>
          <p className="mt-1 text-body text-muted">Os campeões e as skins da sua conta.</p>
        </div>
        <Progress label="Campeões" owned={collection.ownedChampions} total={collection.totalChampions} />
        <Progress label="Skins" owned={collection.ownedSkins} total={collection.totalSkins} />
      </header>
      <section aria-label="Campeões" className="flex min-w-0 flex-col gap-4 rounded-panel surface card p-5 shadow-soft">
        <div className="flex flex-wrap items-center gap-3">
          <SearchField label="Buscar campeão" placeholder="Buscar campeão" value={filters.query} className="min-w-[12rem] flex-1"
            onChange={event => set({ query: event.target.value })}
            onKeyDown={event => { if (event.key === 'Escape') set({ query: '' }); }} />
          <Segmented label="Mostrar" value={filters.ownership} options={ownershipOptions} onChange={ownership => set({ ownership })} />
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <ul aria-label="Filtrar por função" className="flex flex-wrap gap-1">
            {CHAMPION_ROLES.map(role => (
              <li key={role}>
                <button type="button" aria-pressed={filters.role === role}
                  onClick={() => set({ role: filters.role === role ? null : role })}
                  className="h-8 rounded-full border border-control px-3 text-caption font-semibold text-muted hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent aria-pressed:border-accent aria-pressed:bg-accent-soft aria-pressed:text-accent-ink">
                  {roleNames[role]}
                </button>
              </li>
            ))}
          </ul>
          {showLanes && (
            <ul aria-label="Filtrar por rota" className="flex gap-1">
              {LANES.map(lane => (
                <li key={lane}>
                  <button type="button" aria-pressed={filters.lane === lane} title={positionNames[lane]} aria-label={positionNames[lane]}
                    onClick={() => set({ lane: filters.lane === lane ? null : lane })}
                    className="flex h-8 w-8 items-center justify-center rounded-control border border-control text-muted hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent aria-pressed:border-accent aria-pressed:bg-accent-soft aria-pressed:text-accent-ink">
                    <PositionIcon position={lane} size={18} />
                  </button>
                </li>
              ))}
            </ul>
          )}
          <p aria-live="polite" className="ml-auto text-caption text-muted">
            {shown.length} {shown.length === 1 ? 'campeão' : 'campeões'}
          </p>
        </div>
        {shown.length === 0
          ? <p className="py-6 text-center text-body text-muted">Nenhum campeão encontrado.</p>
          : <ul aria-label="Grade de campeões" className="collection-grid">
            {shown.map(c => <li key={c.id}><ChampionTile champion={c} onOpen={() => setOpen(c.id)} /></li>)}
          </ul>}
      </section>
    </div>
  );
}

function Progress({ label, owned, total }: { label: string; owned: number; total: number }) {
  return (
    <div className="flex min-w-[9rem] flex-col gap-1.5">
      <p className="flex items-baseline justify-between gap-3 text-caption font-semibold text-muted">
        <span>{label}</span>
        <span><span className="text-section text-ink">{owned}</span> / {total}</span>
      </p>
      <div role="progressbar" aria-label={`${label}: ${owned} de ${total}`} aria-valuemin={0} aria-valuemax={total} aria-valuenow={owned}
        className="h-1.5 overflow-hidden rounded-full bg-raised">
        <div className="h-full rounded-full bg-accent" style={{ width: `${percent(owned, total)}%` }} />
      </div>
    </div>
  );
}

function ChampionTile({ champion, onOpen }: { champion: CollectionChampion; onOpen: () => void }) {
  const status = champion.owned ? null : champion.freeToPlay ? 'Grátis' : champion.rented ? 'Alugado' : null;
  const description = [
    champion.name,
    champion.owned ? 'na conta' : champion.freeToPlay ? 'na rotação gratuita' : champion.rented ? 'alugado' : 'não está na conta',
    `${champion.ownedSkins} de ${champion.totalSkins} skins`,
  ].join(', ');
  // Card em pé, no formato de um celular (#394): moldura com borda, a tela de carregamento da skin
  // padrão (id do campeão vezes 1000) dentro e o nome embaixo. Sem o ícone quadrado por baixo: ele aparecia
  // através da imagem apagada dos campeões que a conta não tem.
  return (
    <button type="button" aria-label={description} title={description} onClick={onOpen}
      className="group flex w-full flex-col gap-1.5 rounded-card border-2 border-accent/55 bg-raised p-1.5 text-left shadow-soft transition-[transform,border-color] hover:-translate-y-0.5 hover:border-accent focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
      <span className="relative block aspect-[308/560] w-full overflow-hidden rounded-control bg-surface">
        <img src={assetUrl('skin', champion.id * 1000)} alt="" loading="lazy"
          className={cx('absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.04]', !champion.owned && 'opacity-45 grayscale')}
          onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
        {status && <span className="absolute left-1.5 top-1.5 rounded-full bg-accent px-2 py-0.5 text-[0.625rem] font-bold uppercase text-on-accent">{status}</span>}
        {champion.totalSkins > 0 && <span aria-hidden="true" className="absolute bottom-1.5 right-1.5 rounded-full bg-black/65 px-2 py-0.5 text-[0.625rem] font-semibold text-white">
          {champion.ownedSkins}/{champion.totalSkins} skins
        </span>}
      </span>
      <span className={cx('truncate px-0.5 text-body font-semibold', champion.owned ? 'text-ink' : 'text-muted')}>{champion.name}</span>
    </button>
  );
}

/** Skins e cromas de um campeão, pedidos ao abrir. */
function ChampionSkinsView({ state, champion, send, onBack }: {
  state: AgentState; champion: CollectionChampion; send: AgentClient['send']; onBack: () => void;
}) {
  const load = useCommand(send);
  const { run } = load;
  const reload = () => void run({ type: 'openChampionSkins', championId: champion.id }, { pending: 'Carregando as skins…', failure: 'carregar as skins' });
  useEffect(() => {
    reload();
    // `run` muda a cada render; as skins são pedidas de novo só para outro campeão.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [champion.id]);
  const skins = state.championSkins?.championId === champion.id ? state.championSkins.skins : null;
  const [selected, setSelected] = useState<number | null>(null);
  const current = skins?.find(s => s.id === selected) ?? skins?.[0] ?? null;
  const purchased = champion.owned ? purchasedLabel(champion.purchased) : null;
  const back = <button type="button" className="nav-button self-start" onClick={onBack}>Voltar para a Coleção</button>;
  return (
    <div className="flex flex-col gap-4">
      {back}
      <header className="relative isolate flex min-h-[11rem] flex-col justify-end overflow-hidden rounded-panel surface card p-5 shadow-soft animate-fade-up">
        {current && <img src={assetUrl('skinSplash', current.id)} alt="" aria-hidden="true"
          className="absolute inset-0 -z-10 h-full w-full object-cover object-top opacity-60"
          onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />}
        <div className="absolute inset-0 -z-10 bg-gradient-to-t from-surface via-surface/70 to-transparent" aria-hidden="true" />
        <div className="flex flex-wrap items-end gap-4">
          <ChampionIcon id={champion.id} size={64} dim={!champion.owned} />
          <div className="min-w-0">
            <h2 className="text-title font-semibold text-ink">{champion.name}</h2>
            {champion.title && <p className="text-body text-muted first-letter:uppercase">{champion.title}</p>}
            <div className="mt-2 flex flex-wrap gap-2">
              <Tag>{champion.owned ? 'Na conta' : champion.freeToPlay ? 'Rotação gratuita' : champion.rented ? 'Alugado' : 'Não está na conta'}</Tag>
              {champion.roles.map(role => <Tag key={role}>{roleNames[role as keyof typeof roleNames] ?? role}</Tag>)}
              {purchased && <Tag>Desde {purchased}</Tag>}
            </div>
          </div>
        </div>
      </header>
      {!skins
        ? load.feedback.status === 'error'
          ? <EmptyState title="Skins indisponíveis" description={load.feedback.message} action={<Button onClick={reload}>Tentar de novo</Button>} />
          : <p className="flex items-center gap-2 py-6 text-body text-muted"><Spinner size={18} label={null} /> Carregando as skins…</p>
        : <SkinsSection skins={skins} current={current} onSelect={setSelected} />}
    </div>
  );
}

function SkinsSection({ skins, current, onSelect }: { skins: CollectionSkin[]; current: CollectionSkin | null; onSelect: (id: number) => void }) {
  const totals = skinTotals(skins);
  return (
    <section aria-label="Skins" className="flex flex-col gap-4 rounded-panel surface card p-5 shadow-soft">
      <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
        <h3 className="text-section font-semibold text-ink">Skins</h3>
        <p className="text-caption text-muted">
          {totals.ownedSkins} de {totals.totalSkins} skins
          {totals.totalChromas > 0 && ` · ${totals.ownedChromas} de ${totals.totalChromas} cromas`}
        </p>
      </div>
      <ul aria-label="Skins do campeão" className="skin-grid">
        {skins.map(skin => {
          const rarity = rarityName(skin.rarity);
          return (
            <li key={skin.id}>
              <button type="button" aria-pressed={current?.id === skin.id} onClick={() => onSelect(skin.id)}
                aria-label={`${skin.name}${skin.isBase ? ', padrão' : ''}${rarity ? `, ${rarity}` : ''}, ${skin.owned ? 'na conta' : 'não está na conta'}`}
                className="group flex w-full flex-col gap-1.5 rounded-card border-2 border-transparent p-1 text-left focus-visible:outline-2 focus-visible:outline-accent aria-pressed:border-accent">
                <span className="relative block aspect-[308/560] w-full overflow-hidden rounded-control bg-raised">
                  <img src={assetUrl('skin', skin.id)} alt="" loading="lazy"
                    className={cx('h-full w-full object-cover transition-transform group-hover:scale-[1.03]', !skin.owned && 'opacity-40 grayscale')}
                    onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
                  {!skin.owned && <span className="absolute left-1.5 top-1.5 rounded-full bg-surface/90 px-2 py-0.5 text-[0.625rem] font-bold uppercase text-muted">Não tenho</span>}
                  {skin.chromas.length > 0 && <span className="absolute bottom-1.5 right-1.5 rounded-full bg-surface/90 px-2 py-0.5 text-[0.625rem] font-semibold text-ink">
                    {skin.chromas.length} {skin.chromas.length === 1 ? 'croma' : 'cromas'}
                  </span>}
                </span>
                <span className="flex flex-col px-0.5">
                  <span className={cx('line-clamp-2 text-caption font-semibold', skin.owned ? 'text-ink' : 'text-muted')}>{skin.name}</span>
                  {rarity && <span aria-hidden="true" className="text-[0.6875rem] font-semibold uppercase tracking-wide text-accent-ink">{rarity}</span>}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      {current && current.chromas.length > 0 && (
        <div className="flex flex-col gap-3 border-t border-line pt-4">
          <h4 className="text-body font-semibold text-ink">Cromas de {current.name}</h4>
          <ul aria-label={`Cromas de ${current.name}`} className="flex flex-wrap gap-3">
            {current.chromas.map(chroma => (
              <li key={chroma.id} title={`${chroma.name}${chroma.owned ? '' : ' (não tenho)'}`}
                className="flex w-24 flex-col items-center gap-1 text-center">
                <span className="relative block size-20 overflow-hidden rounded-control bg-raised">
                  <img src={assetUrl('skin', chroma.id)} alt="" loading="lazy" className={cx('h-full w-full object-contain', !chroma.owned && 'opacity-40 grayscale')}
                    onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
                  {chroma.colors[0] && <span aria-hidden="true" className="absolute bottom-1 right-1 size-3.5 rounded-full border border-surface"
                    style={{ background: chroma.colors[1] && chroma.colors[1] !== chroma.colors[0]
                      ? `linear-gradient(135deg, ${chroma.colors[0]} 50%, ${chroma.colors[1]} 50%)` : chroma.colors[0] }} />}
                </span>
                <span className={cx('text-caption', chroma.owned ? 'font-semibold text-ink' : 'text-muted')}>
                  {chroma.name || 'Croma'}
                  <span className="sr-only">{chroma.owned ? ', na conta' : ', não está na conta'}</span>
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function Tag({ children }: { children: ReactNode }) {
  return <span className="w-fit rounded-full bg-raised px-3 py-1 text-caption font-semibold text-muted">{children}</span>;
}

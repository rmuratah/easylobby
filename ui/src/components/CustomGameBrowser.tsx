import { useEffect, useId, useRef, useState } from 'react';
import { filterListings, listingFull, listingMapName, listingModeName } from '../agent/customGame';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { CustomGameListing } from '../bindings/CustomGameListing';
import { SearchField } from './base/TextField';
import { cx } from './base/cx';

type Props = {
  /** A lista do agente; `null` antes da primeira busca. */
  games: CustomGameListing[] | null;
  selectedId: number | null;
  onSelect: (id: number) => void;
  locked: boolean;
  send: AgentClient['send'];
};

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const head = 'text-caption font-semibold uppercase tracking-wider text-muted';

/**
 * Aba "Entrar" da Personalizada (#264): as personalizadas abertas, com busca
 * pelo nome da partida ou do dono e o botão de atualizar. Escolher uma linha
 * leva a partida para o rodapé, onde fica o botão de entrar (e a senha, se a
 * partida tiver). Partidas cheias aparecem, mas não podem ser escolhidas.
 */
export function CustomGameBrowser({ games, selectedId, onSelect, locked, send }: Props) {
  const baseId = useId();
  const [query, setQuery] = useState('');
  const refresh = useCommand(send);
  const runRefresh = refresh.run;
  // A primeira visita à aba busca a lista sozinha.
  const asked = useRef(false);
  useEffect(() => {
    if (asked.current || games !== null) return;
    asked.current = true;
    void runRefresh({ type: 'refreshCustomGames' }, { pending: 'Buscando personalizadas…', failure: 'buscar as personalizadas' });
  }, [games, runRefresh]);

  const shown = games ? filterListings(games, query) : [];
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end gap-3">
        <SearchField label="Buscar personalizada" placeholder="Buscar por nome ou dono" value={query}
          onChange={event => setQuery(event.target.value)} className="min-w-[12rem] flex-1" />
        <button type="button" disabled={refresh.pending} aria-busy={refresh.pending}
          onClick={() => { void refresh.run({ type: 'refreshCustomGames' }, { pending: 'Atualizando a lista…', failure: 'atualizar a lista' }); }}
          className={`inline-flex h-10 items-center justify-center gap-2 rounded-full px-4 text-body font-semibold surface shadow-soft transition-all enabled:hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:text-muted ${focusRing}`}>
          <RefreshIcon /> Atualizar
        </button>
      </div>
      {refresh.feedback.message && (
        <p role={refresh.feedback.status === 'error' ? 'alert' : 'status'}
          className={refresh.feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{refresh.feedback.message}</p>
      )}

      {games === null ? (
        refresh.pending ? null : <p className="text-body text-muted">A lista ainda não foi carregada. Use Atualizar.</p>
      ) : !games.length ? (
        <p className="text-body text-muted">Nenhuma personalizada aberta agora.</p>
      ) : !shown.length ? (
        <p className="text-body text-muted">Nenhuma personalizada encontrada para “{query.trim()}”.</p>
      ) : (
        <fieldset aria-label="Personalizadas abertas" disabled={locked} className="flex flex-col gap-1.5">
          <div aria-hidden="true" className="hidden grid-cols-[minmax(0,2fr)_minmax(0,1.2fr)_minmax(0,1.2fr)_minmax(0,1.2fr)_4.5rem_4.5rem_1.5rem] gap-3 px-3 @min-[48rem]:grid">
            <span className={head}>Nome</span><span className={head}>Dono</span><span className={head}>Mapa</span>
            <span className={head}>Modo</span><span className={head}>Jogadores</span><span className={head}>Espect.</span><span />
          </div>
          {shown.map(game => {
            const full = listingFull(game);
            const selected = game.id === selectedId;
            return (
              <label key={game.id} title={full ? 'Partida cheia' : undefined}
                className={cx('grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-3 gap-y-0.5 rounded-tile px-3 py-2.5 transition-colors',
                  '@min-[48rem]:grid-cols-[minmax(0,2fr)_minmax(0,1.2fr)_minmax(0,1.2fr)_minmax(0,1.2fr)_4.5rem_4.5rem_1.5rem]',
                  selected ? 'bg-accent-soft ring-2 ring-accent' : 'surface',
                  full || locked ? 'cursor-not-allowed' : 'cursor-pointer hover:bg-raised', full && 'opacity-60')}>
                <input type="radio" name={`${baseId}-game`} className="sr-only" checked={selected} disabled={full}
                  onChange={() => onSelect(game.id)}
                  aria-label={`${game.name || 'Sem nome'}, de ${game.owner || 'dono desconhecido'}${game.hasPassword ? ', com senha' : ''}${full ? ', cheia' : ''}`} />
                <span className="truncate text-body font-semibold">{game.name || 'Sem nome'}</span>
                <span className="text-right text-caption text-muted @min-[48rem]:hidden">
                  {game.players}/{game.maxPlayers}{game.hasPassword && ' · com senha'}
                </span>
                <span className="col-span-2 truncate text-caption text-muted @min-[48rem]:hidden">
                  {[game.owner, listingMapName(game), listingModeName(game)].filter(Boolean).join(' · ')}
                </span>
                <span className="hidden truncate text-body @min-[48rem]:block">{game.owner}</span>
                <span className="hidden truncate text-body @min-[48rem]:block">{listingMapName(game)}</span>
                <span className="hidden truncate text-body @min-[48rem]:block">{listingModeName(game)}</span>
                <span className="hidden text-body tabular-nums @min-[48rem]:block">{game.players}/{game.maxPlayers}</span>
                <span className="hidden text-body tabular-nums @min-[48rem]:block">{game.spectators}/{game.maxSpectators}</span>
                <span className="hidden text-muted @min-[48rem]:flex" title={game.hasPassword ? 'Com senha' : undefined}>
                  {game.hasPassword && <LockIcon />}
                </span>
              </label>
            );
          })}
        </fieldset>
      )}
    </div>
  );
}

function RefreshIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M20 11a8 8 0 0 0-14.9-3M4 4v4h4" />
      <path d="M4 13a8 8 0 0 0 14.9 3M20 20v-4h-4" />
    </svg>
  );
}

function LockIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="5" y="11" width="14" height="10" rx="2" />
      <path d="M8 11V7a4 4 0 0 1 8 0v4" />
    </svg>
  );
}

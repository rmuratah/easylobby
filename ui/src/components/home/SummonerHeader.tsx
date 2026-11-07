import { assetUrl } from '../../agent/assets';
import type { Summoner } from '../../bindings/Summoner';
import type { RankedStats } from '../../bindings/RankedStats';
import type { RankedEntry } from '../../bindings/RankedEntry';
import { rankLabel } from '../../agent/profile';
import { Avatar } from '../base';

/**
 * Quem está logado: ícone, Riot ID e nível (#128). No início, ocupa o cartão
 * ao lado do principal, com capa decorada e o ícone sobre ela.
 */
export function SummonerHeader({ summoner, ranked }: { summoner: Summoner; ranked: RankedStats | null }) {
  const name = summoner.gameName || summoner.displayName || 'Invocador';
  return (
    <section aria-label="Invocador logado"
      className="group relative flex h-full flex-col items-center overflow-hidden rounded-panel surface pb-5 shadow-soft text-center animate-fade-up"
      style={{ animationDelay: '80ms' }}>
      <div aria-hidden="true" className="h-20 w-full shrink-0 bg-hero-art" />
      <div className="relative -mt-10 rounded-full bg-primary-gradient p-1 shadow-soft transition-transform duration-500 group-hover:scale-105">
        <Avatar name={name} src={assetUrl('profileIcon', summoner.profileIconId)} size={80} className="ring-2 ring-surface" />
        <span role="img" aria-label="Presença indisponível" title="Presença indisponível"
          className="absolute bottom-1 right-1 size-4 rounded-full border-2 border-surface bg-control" />
      </div>
      <div className="mt-3 flex min-w-0 max-w-full flex-col items-center gap-1.5 px-5">
        <h2 className="max-w-full break-words text-title font-semibold text-ink">
          {name}{summoner.gameName && summoner.tagLine && <span className="font-medium text-muted">#{summoner.tagLine}</span>}
        </h2>
        <p className="rounded-full bg-raised px-3 py-1 text-caption font-semibold text-muted">Nível {summoner.summonerLevel}</p>
      </div>
      <dl className="mx-5 mt-3 grid w-[calc(100%-2.5rem)] grid-cols-2 gap-2 text-left">
        <div className="rounded-tile bg-raised p-3">
          <dt className="text-caption font-semibold text-muted">Solo/Duo</dt>
          <dd className="mt-1 text-body font-semibold text-ink">{rank(ranked?.soloDuo)}</dd>
        </div>
        <div className="rounded-tile bg-raised p-3">
          <dt className="text-caption font-semibold text-muted">Flex</dt>
          <dd className="mt-1 text-body font-semibold text-ink">{rank(ranked?.flex)}</dd>
        </div>
      </dl>
    </section>
  );
}

/** Elo e PDL de uma fila, ou `Indisponível` enquanto o agente não mandou os ranks. */
function rank(entry: RankedEntry | undefined): string {
  if (!entry) return 'Indisponível';
  return entry.tier ? `${rankLabel(entry)} · ${entry.leaguePoints} PDL` : rankLabel(entry);
}

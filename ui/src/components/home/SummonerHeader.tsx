import { assetUrl } from '../../agent/assets';
import type { Summoner } from '../../bindings/Summoner';
import { Avatar } from '../base';

/**
 * Quem está logado: ícone, Riot ID e nível (#128). No início, ocupa o cartão
 * ao lado do principal, com capa decorada e o ícone sobre ela.
 */
export function SummonerHeader({ summoner }: { summoner: Summoner }) {
  const name = summoner.gameName || summoner.displayName || 'Invocador';
  return (
    <section aria-label="Invocador logado"
      className="group relative flex h-full flex-col items-center overflow-hidden rounded-panel surface pb-6 shadow-soft text-center animate-fade-up"
      style={{ animationDelay: '80ms' }}>
      <div aria-hidden="true" className="h-24 w-full shrink-0 bg-hero-art" />
      <div className="-mt-12 rounded-full bg-primary-gradient p-1 shadow-soft transition-transform duration-500 group-hover:scale-105">
        <Avatar name={name} src={assetUrl('profileIcon', summoner.profileIconId)} size={88} className="ring-2 ring-surface" />
      </div>
      <div className="mt-4 flex min-w-0 max-w-full flex-col items-center gap-2 px-5">
        <h2 className="max-w-full break-words text-title font-semibold text-ink">
          {name}{summoner.gameName && summoner.tagLine && <span className="font-medium text-muted">#{summoner.tagLine}</span>}
        </h2>
        <p className="rounded-full bg-raised px-3 py-1 text-caption font-semibold text-muted">Nível {summoner.summonerLevel}</p>
      </div>
    </section>
  );
}

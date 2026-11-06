import { assetUrl } from '../../agent/assets';
import type { Summoner } from '../../bindings/Summoner';
import { Avatar } from '../base';

/** Quem está logado: ícone, Riot ID e nível, no alto do início (#128). */
export function SummonerHeader({ summoner }: { summoner: Summoner }) {
  const name = summoner.gameName || summoner.displayName || 'Invocador';
  return (
    <section aria-label="Invocador logado" className="flex items-center gap-4">
      <Avatar name={name} src={assetUrl('profileIcon', summoner.profileIconId)} size={64} />
      <div className="min-w-0">
        <h2 className="break-words text-title font-semibold tracking-[-0.015em] text-ink">
          {name}{summoner.gameName && summoner.tagLine && <span className="font-medium text-muted">#{summoner.tagLine}</span>}
        </h2>
        <p className="text-body text-muted">Nível {summoner.summonerLevel}</p>
      </div>
    </section>
  );
}

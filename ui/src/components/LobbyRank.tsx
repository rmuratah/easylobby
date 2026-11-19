import { useState } from 'react';
import type { RankedStats } from '../bindings/RankedStats';
import { assetUrl } from '../agent/assets';
import { lobbyRankQueue, rankLabel, tierEmblemId, winRate } from '../agent/profile';
import { cx } from './base/cx';

type Props = { ranked: RankedStats | null; queueId: number };

/** Elo do jogador no card do lobby (#275), sempre numa linha só (#605). Some enquanto o agente ainda busca os ranks. */
export function LobbyRank({ ranked, queueId }: Props) {
  const [failed, setFailed] = useState<string | null>(null);
  if (!ranked) return null;
  const queue = lobbyRankQueue(queueId);
  const entry = queue === 'flex' ? ranked.flex : ranked.soloDuo;
  const queueName = queue === 'flex' ? 'Flex' : 'Solo/Duo';
  const id = entry.tier ? tierEmblemId(entry.tier) : null;
  const src = id ? assetUrl('rankedEmblem', id) : null;
  const label = rankLabel(entry);
  const points = entry.tier ? ` · ${entry.leaguePoints} PDL` : '';
  return (
    <span title={`Elo na ${queueName}`}
      className={cx('inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full py-1 pl-1 pr-3 text-caption font-semibold @min-[60rem]:text-body',
        entry.tier ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-muted')}>
      {src && failed !== src && (
        // O emblema do CommunityDragon tem muita borda transparente: a imagem fica maior que a caixa.
        <span aria-hidden="true" className="relative size-7 shrink-0">
          <img src={src} alt="" width={56} height={56} onError={() => setFailed(src)}
            className="pointer-events-none absolute left-1/2 top-1/2 size-14 max-w-none -translate-x-1/2 -translate-y-1/2 object-contain" />
        </span>
      )}
      <span className="sr-only">{queueName}: </span>
      {label}{points}
    </span>
  );
}

/** Vitórias na mesma fila do elo, embaixo dele no card do lobby (#607). Some sem partidas. */
export function LobbyWinRate({ ranked, queueId }: Props) {
  if (!ranked) return null;
  const entry = lobbyRankQueue(queueId) === 'flex' ? ranked.flex : ranked.soloDuo;
  const rate = winRate(entry.wins, entry.losses);
  if (rate === null) return null;
  return (
    <p className="whitespace-nowrap text-caption text-muted tabular-nums @min-[60rem]:text-body">
      <span className={rate >= 50 ? 'font-semibold text-accent-ink' : 'font-semibold text-ink'}>{rate}%</span> de vitórias · {entry.wins}V {entry.losses}D
    </p>
  );
}

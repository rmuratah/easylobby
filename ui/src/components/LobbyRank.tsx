import { useState } from 'react';
import type { RankedStats } from '../bindings/RankedStats';
import { assetUrl } from '../agent/assets';
import { lobbyRankQueue, rankLabel, tierEmblemId } from '../agent/profile';
import { cx } from './base/cx';

type Props = { ranked: RankedStats | null; queueId: number };

/** Elo do jogador no card do lobby (#275). Some enquanto o agente ainda busca os ranks. */
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
      className={cx('mt-2 inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-caption font-semibold',
        entry.tier ? 'bg-accent-soft text-accent-ink' : 'bg-raised text-muted')}>
      {src && failed !== src && (
        // O emblema do CommunityDragon tem muita borda transparente: a imagem fica maior que a caixa.
        <span aria-hidden="true" className="relative size-4 shrink-0">
          <img src={src} alt="" width={32} height={32} onError={() => setFailed(src)}
            className="pointer-events-none absolute left-1/2 top-1/2 size-8 max-w-none -translate-x-1/2 -translate-y-1/2 object-contain" />
        </span>
      )}
      <span className="sr-only">{queueName}: </span>
      {label}{points}
    </span>
  );
}

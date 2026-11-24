import { useState } from 'react';
import { assetUrl } from '../agent/assets';
import { tierEmblemId, tierName } from '../agent/profile';
import { cx } from './base/cx';

type Size = 'md' | 'lg';

// A imagem do CommunityDragon tem muita borda transparente: ela fica com
// o dobro do tamanho da caixa, centralizada, e a borda sobra para fora.
const sizes: Record<Size, { box: string; img: string; px: number; fallback: string }> = {
  md: { box: 'size-20', img: 'size-40', px: 160, fallback: 'size-14' },
  lg: { box: 'size-28', img: 'size-56', px: 224, fallback: 'size-16' },
};

/**
 * Emblema do elo (perfil e card do invocador, #659); sem classificação, ou se
 * a imagem não carregar, um círculo com a sigla do tier.
 */
export function TierEmblem({ tier, size = 'lg', className }: { tier: string | null; size?: Size; className?: string }) {
  const [failed, setFailed] = useState<string | null>(null);
  const s = sizes[size];
  const id = tier ? tierEmblemId(tier) : null;
  const src = id ? assetUrl('rankedEmblem', id) : null;
  if (tier && src && failed !== src) {
    return (
      <span aria-hidden="true" data-testid="tier-emblem" className={cx('relative shrink-0', s.box, className)}>
        <img src={src} alt="" width={s.px} height={s.px} onError={() => setFailed(src)}
          className={cx('pointer-events-none absolute left-1/2 top-1/2 max-w-none -translate-x-1/2 -translate-y-1/2 object-contain', s.img)} />
      </span>
    );
  }
  return (
    <span aria-hidden="true" data-testid="tier-emblem"
      className={cx('flex shrink-0 items-center justify-center rounded-full border-2 text-caption font-bold uppercase', s.fallback, className,
        tier ? 'border-accent bg-accent-soft text-accent-ink' : 'border-line bg-raised text-muted')}>
      {tier ? tierName(tier).slice(0, 3) : '—'}
    </span>
  );
}

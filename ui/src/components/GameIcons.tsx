import { useState } from 'react';
import { assetUrl } from '../agent/assets';

/** Ícone pequeno (feitiço, runa, item); espaço vazio ou imagem que não carregou viram um quadrado neutro. */
export function GameIcon({ kind, id, size = 20 }: { kind: 'summonerSpell' | 'perk' | 'perkStyle' | 'item'; id: number; size?: number }) {
  const [failed, setFailed] = useState<string | null>(null);
  const src = id ? assetUrl(kind, id) : null;
  if (!src || failed === src) return <span className="shrink-0 rounded-control-sm bg-raised" style={{ width: size, height: size }} />;
  return <img src={src} alt="" width={size} height={size} loading="lazy" onError={() => setFailed(src)}
    className="shrink-0 rounded-control-sm bg-raised" />;
}

/** Os sete espaços de item de um jogador (o último é o amuleto). */
export function ItemRow({ items, size = 24 }: { items: number[]; size?: number }) {
  return (
    <div aria-hidden="true" className="flex gap-0.5">
      {items.map((item, i) => <GameIcon key={i} kind="item" id={item} size={size} />)}
    </div>
  );
}

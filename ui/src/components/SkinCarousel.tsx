import { useEffect, useRef, useState } from 'react';
import { assetUrl } from '../agent/assets';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { AgentState } from '../bindings/AgentState';
import type { CollectionSkin } from '../bindings/CollectionSkin';
import { cx } from './base/cx';

type Props = {
  state: AgentState;
  send: AgentClient['send'];
  championId: number;
  championName: string;
  /** `selectedSkinId` do jogador; `null` ou `0` é a skin padrão. */
  selectedSkinId: number | null;
};

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';

/**
 * Escolha da skin depois de confirmar o campeão (#477): a splash art grande
 * da skin em cima e as miniaturas embaixo, num carrossel. Clicar numa
 * miniatura troca a skin na LCU. As que a conta não tem ficam esmaecidas e,
 * clicadas, só mostram a splash grande, sem trocar a skin (#494).
 */
export function SkinCarousel({ state, send, championId, championName, selectedSkinId }: Props) {
  const load = useCommand(send);
  const choose = useCommand(send);
  const { run } = load;
  const rowRef = useRef<HTMLUListElement>(null);
  // A skin clicada aparece na hora; depois vale a que a LCU confirmou.
  const [pending, setPending] = useState<number | null>(null);
  // Skin que a conta não tem, aberta só para ver a splash.
  const [preview, setPreview] = useState<number | null>(null);
  useEffect(() => {
    void run({ type: 'openChampionSkins', championId }, { pending: '', failure: 'carregar as skins' });
    // `run` muda a cada render; as skins são pedidas de novo só para outro campeão.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [championId]);
  const loaded = state.championSkins?.championId === championId;
  const skins: CollectionSkin[] = loaded ? state.championSkins!.skins : [];
  const confirmed = selectedSkinId || championId * 1000;
  const selected = choose.pending && pending ? pending : confirmed;
  const previewing = preview !== null && skins.some(skin => skin.id === preview && !skin.owned) ? preview : null;
  const shown = previewing ?? selected;
  const current = skins.find(skin => skin.id === shown) ?? skins.find(skin => skin.isBase) ?? null;
  // A miniatura marcada fica à vista quando a lista abre ou a skin muda.
  useEffect(() => {
    const row = rowRef.current;
    const item = row?.querySelector<HTMLElement>('[aria-pressed="true"]');
    if (!row || !item) return;
    row.scrollLeft = item.offsetLeft - (row.clientWidth - item.offsetWidth) / 2;
  }, [selected, loaded]);
  function pick(skin: CollectionSkin) {
    if (!skin.owned) {
      setPreview(skin.id === previewing ? null : skin.id);
      return;
    }
    setPreview(null);
    if (skin.id === confirmed || choose.pending) return;
    setPending(skin.id);
    void choose.run({ type: 'selectSkin', skinId: skin.id }, { pending: '', failure: 'trocar a skin' });
  }
  function scroll(direction: 1 | -1) {
    const row = rowRef.current;
    if (row) row.scrollBy({ left: direction * row.clientWidth * 0.8, behavior: 'smooth' });
  }
  const error = [load.feedback, choose.feedback].find(f => f.status === 'error');
  return (
    // Mesmo tamanho do card da grade de campeões (#477): a splash ocupa o que sobra.
    <section aria-label="Skins" className="champ-select-picker flex min-w-0 flex-col gap-3 rounded-card border border-edge bg-surface card p-4 text-ink">
      <h3 className="text-section font-semibold">Escolha a skin de {championName}</h3>
      <div className="relative min-h-0 w-full flex-1 overflow-hidden rounded-control bg-raised">
        <img key={shown} src={assetUrl('skinSplash', shown)} alt="" aria-hidden="true"
          className="h-full w-full object-cover animate-fade-up"
          onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
        {current && (
          <p role="status" className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/75 to-transparent px-4 pb-3 pt-10 text-section font-semibold text-white">
            {current.isBase ? championName : current.name}
            {previewing !== null && <span className="ml-2 text-body font-normal text-white/80">· Você não tem esta skin</span>}
          </p>
        )}
      </div>
      {!loaded ? (
        <p className={cx('text-body', error ? 'text-danger' : 'text-muted')}>{error ? error.message : 'Carregando as skins…'}</p>
      ) : (
        <div className="flex shrink-0 items-center gap-2">
          <ArrowButton label="Skins anteriores" direction={-1} onClick={() => scroll(-1)} />
          <ul ref={rowRef} aria-label="Miniaturas das skins" className="flex min-w-0 flex-1 gap-2 overflow-x-auto scroll-smooth pb-1 [justify-content:safe_center]">
            {skins.map(skin => {
              const name = skin.isBase ? `${championName} (padrão)` : skin.name;
              const viewing = previewing === skin.id;
              return (
                <li key={skin.id} className="shrink-0">
                  <button type="button" aria-pressed={selected === skin.id} aria-current={viewing || undefined}
                    aria-label={skin.owned ? name : `${name} (você não tem; só ver)`}
                    title={skin.owned ? name : `${name}: você não tem esta skin. Clique para ver a splash.`}
                    onClick={() => pick(skin)}
                    className={cx('block w-28 overflow-hidden rounded-control border-2 border-transparent hover:border-control aria-pressed:border-accent',
                      viewing && 'border-dashed !border-muted', focusRing)}>
                    <span className={cx('block aspect-[1215/717] w-full bg-raised', !skin.owned && !viewing && 'opacity-40 grayscale', !skin.owned && viewing && 'grayscale')}>
                      <img src={assetUrl('skinSplash', skin.id)} alt="" loading="lazy" className="h-full w-full object-cover"
                        onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
          <ArrowButton label="Próximas skins" direction={1} onClick={() => scroll(1)} />
        </div>
      )}
      {loaded && error && <p role="alert" className="text-body text-danger">{error.message}</p>}
    </section>
  );
}

function ArrowButton({ label, direction, onClick }: { label: string; direction: 1 | -1; onClick: () => void }) {
  return (
    <button type="button" aria-label={label} title={label} onClick={onClick}
      className={cx('flex size-9 shrink-0 items-center justify-center rounded-full border border-control text-muted hover:bg-raised hover:text-ink', focusRing)}>
      <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"
        strokeLinecap="round" strokeLinejoin="round"><path d={direction === 1 ? 'M9 6l6 6-6 6' : 'M15 6l-6 6 6 6'} /></svg>
    </button>
  );
}

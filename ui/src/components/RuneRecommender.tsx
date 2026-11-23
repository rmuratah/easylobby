import { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import { LANES, positionNames, type Lane } from '../agent/positions';
import { recommendationPerks, recommendationTitle, recommenderLane } from '../agent/runeRecommender';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { RecommendedRunePage } from '../bindings/RecommendedRunePage';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import { Segmented, Spinner } from './base';
import { cx } from './base/cx';
import { PositionIcon } from './PositionIcon';
import { RuneIcon } from './RuneEditor';
import { SpellIcon } from './SummonerSpellsPicker';

type Props = {
  state: AgentState;
  champSelect: ChampSelect;
  /** Campeão escolhido ou marcado; sem ele, o botão fica desabilitado (#647). */
  championId: number | null;
  send: AgentClient['send'];
  /** Abre o editor de runas, para "Editar a página depois de aplicar". */
  onEdit?: () => void;
};

/**
 * Botão "Recomendador" da seleção (#596), como o do client oficial: abre as
 * páginas de runas que o client sugere para o campeão na rota. Escolher uma
 * a aplica como página temporária, sem gastar página comprada.
 */
export function RuneRecommenderButton({ championId, ...props }: Props) {
  const [open, setOpen] = useState(false);
  // Sem campeão, o botão continua no lugar, desabilitado e com a explicação (#647).
  // aria-disabled (e não disabled) mantém o foco e o tooltip no botão.
  const disabled = championId === null;
  return (
    <>
      <button type="button" aria-label="Recomendador de runas" aria-haspopup="dialog" aria-disabled={disabled || undefined}
        title={disabled ? 'Escolha um campeão para ver as runas recomendadas' : 'Recomendador de runas'}
        onClick={() => { if (!disabled) setOpen(true); }}
        className={cx('flex size-9 shrink-0 items-center justify-center rounded-control border border-control text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
          disabled ? 'cursor-not-allowed opacity-50' : 'hover:bg-raised hover:text-ink')}>
        <RecommenderIcon />
      </button>
      {open && championId !== null && <RuneRecommenderDialog {...props} championId={championId} onClose={() => setOpen(false)} />}
    </>
  );
}

function RuneRecommenderDialog({ state, champSelect, championId, send, onEdit, onClose }: Omit<Props, 'championId'> & { championId: number; onClose: () => void }) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  useEffect(() => { closeRef.current = onClose; });
  const [lane, setLane] = useState<Lane>(() => recommenderLane(champSelect, state.championCatalog, championId));
  const [editAfter, setEditAfter] = useState(false);
  const load = useCommand(send);
  const apply = useCommand(send);
  const loadRef = useRef(load.run);
  useEffect(() => { loadRef.current = load.run; });

  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>('button')?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') closeRef.current(); }
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      opener?.focus?.();
    };
  }, []);

  // Pede as sugestões ao abrir e a cada troca de rota ou de campeão.
  useEffect(() => {
    void loadRef.current({ type: 'openRecommendedRunes', championId, position: lane },
      { pending: 'Carregando as sugestões…', failure: 'carregar as sugestões' });
  }, [championId, lane]);

  const recommended = state.recommendedRunes;
  const pages = recommended?.championId === championId && recommended.position === lane ? recommended.pages : null;
  const championName = state.championCatalog?.champions.find(champion => champion.id === championId)?.name;

  async function choose(index: number) {
    if (apply.pending) return;
    const ok = await apply.run({ type: 'applyRecommendedRunes', index },
      { pending: 'Aplicando a página…', failure: 'aplicar a página' });
    if (!ok) return;
    onClose();
    if (editAfter) onEdit?.();
  }

  const feedback = apply.feedback.status !== 'idle' ? apply.feedback : load.feedback.status === 'error' ? load.feedback : null;
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4 animate-fade-up"
      onPointerDown={event => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby={titleId}
        className="flex max-h-[min(40rem,calc(100vh-2rem))] w-full max-w-4xl flex-col overflow-hidden rounded-panel bg-surface card text-ink shadow-lift">
        <header className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-3">
          <div className="flex min-w-0 items-center gap-2">
            <span className="text-accent-ink"><RecommenderIcon /></span>
            <h2 id={titleId} className="text-section font-semibold">
              Recomendador{championName && <span className="font-normal text-muted"> · {championName}</span>}
            </h2>
          </div>
          <div className="flex items-center gap-2">
            <Segmented label="Rota das sugestões" value={lane} onChange={setLane} appearance="embedded"
              options={LANES.map(value => ({ value, ariaLabel: positionNames[value], label: <PositionIcon position={value} size={18} /> }))} />
            <button type="button" aria-label="Fechar" title="Fechar" onClick={onClose}
              className="flex size-8 shrink-0 items-center justify-center rounded-full text-muted hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
              <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
                strokeLinecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
            </button>
          </div>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto p-5">
          {pages === null
            ? (load.feedback.status === 'error'
              ? <p className="text-muted">Sem sugestões agora. Troque a rota ou tente de novo.</p>
              : <p className="flex items-center gap-2 text-muted"><Spinner size={18} label="Carregando as sugestões" /> Carregando as sugestões…</p>)
            : pages.length === 0
              ? <p className="text-muted">O client não tem sugestões para {championName ?? 'este campeão'} como {positionNames[lane]}.</p>
              : (
                <ul aria-label="Páginas sugeridas" className="grid gap-3 sm:grid-cols-3">
                  {pages.map((page, index) => (
                    <li key={index} className="flex">
                      <RecommendationCard page={page} catalog={state.runeCatalog} disabled={apply.pending}
                        onChoose={() => { void choose(index); }} />
                    </li>
                  ))}
                </ul>
              )}
        </div>
        <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-line px-5 py-3">
          {onEdit
            ? (
              <label className="flex items-center gap-2 text-body text-muted">
                <input type="checkbox" checked={editAfter} onChange={event => setEditAfter(event.target.checked)}
                  className="size-4 accent-[var(--el-accent)]" />
                Editar a página depois de aplicar
              </label>
            )
            : <span />}
          {feedback && (
            <p role={feedback.status === 'error' ? 'alert' : 'status'}
              className={feedback.status === 'error' ? 'text-danger' : 'text-muted'}>{feedback.message}</p>
          )}
        </footer>
      </div>
    </div>,
    document.body,
  );
}

function RecommendationCard({ page, catalog, disabled, onChoose }: {
  page: RecommendedRunePage;
  catalog: RuneCatalog | null;
  disabled: boolean;
  onChoose: () => void;
}) {
  const title = recommendationTitle(page, catalog);
  const perks = recommendationPerks(page);
  const perkName = (id: number) => catalog?.perks.find(perk => perk.id === id)?.name ?? '';
  return (
    <button type="button" disabled={disabled} onClick={onChoose} aria-label={`Usar a página ${title}`}
      className={cx('flex w-full flex-col items-center gap-3 rounded-card border border-edge bg-surface p-4 text-center',
        'hover:border-accent hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:opacity-60')}>
      <span title={perkName(perks.keystone)}><RuneIcon kind="perk" id={perks.keystone} size={64} /></span>
      <span className="flex gap-1.5" title={perks.primary.map(perkName).filter(Boolean).join(', ')}>
        {perks.primary.map((id, i) => <RuneIcon key={i} kind="perk" id={id} size={28} />)}
      </span>
      <span className="text-body font-semibold text-ink">{title}</span>
      <span className="flex items-center gap-1.5" title={perks.secondary.map(perkName).filter(Boolean).join(', ')}>
        <RuneIcon kind="perkStyle" id={page.subStyleId} size={20} />
        {perks.secondary.map((id, i) => <RuneIcon key={i} kind="perk" id={id} size={28} />)}
      </span>
      {page.summonerSpellIds.length > 0 && (
        <span className="flex gap-1.5" aria-hidden="true">
          {page.summonerSpellIds.slice(0, 2).map(id => <SpellIcon key={id} id={id} size={28} />)}
        </span>
      )}
      <span className="mt-auto text-caption font-semibold text-accent-ink">Usar esta página</span>
    </button>
  );
}

function RecommenderIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3l9 9-9 9-9-9 9-9z" /><path d="M12 8l4 4-4 4" /><path d="M8 12h8" />
    </svg>
  );
}

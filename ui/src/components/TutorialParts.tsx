import { TUTORIAL_LOCKED, TUTORIAL_REWARD_XP, type TutorialCard } from '../agent/training';
import { cx } from './base/cx';

type Props = {
  cards: TutorialCard[];
  selected: number;
  locked: boolean;
  name: string;
  onSelect: (queueId: number) => void;
};

/**
 * As três partes do Tutorial em cards (#228): número, nome, descrição e
 * recompensa. Parte com cadeado fica desabilitada com o aviso; as concluídas
 * podem ser jogadas de novo.
 */
export function TutorialParts({ cards, selected, locked, name, onSelect }: Props) {
  return (
    <fieldset className="grid min-w-0 gap-3 @min-[40rem]:grid-cols-3">
      <legend className="sr-only">Parte do Tutorial</legend>
      {cards.map(card => {
        const checked = card.queueId === selected;
        const disabled = locked || !card.available;
        const descId = `${name}-${card.queueId}-desc`;
        return (
          <label key={card.queueId} className={cx('relative flex min-h-44 flex-col gap-2 rounded-panel p-5 transition-all',
            !card.available ? 'cursor-not-allowed bg-raised/60 opacity-70'
              : checked ? 'bg-accent-soft ring-2 ring-accent' : 'surface shadow-soft hover:-translate-y-0.5 hover:shadow-lift',
            card.available && (locked ? 'cursor-not-allowed' : 'cursor-pointer'))}>
            <input type="radio" name={name} className="sr-only" checked={checked} disabled={disabled}
              onChange={() => onSelect(card.queueId)} aria-describedby={descId} />
            <span className="flex items-center justify-between gap-2">
              <span className="text-caption font-semibold uppercase tracking-wider text-accent-ink">Parte {card.part}</span>
              {card.completed && <span className="rounded-full bg-accent-soft px-2 py-0.5 text-caption font-semibold text-accent-ink">Concluída</span>}
              {!card.available && <span aria-hidden="true" title="Bloqueada" className="text-muted"><LockIcon /></span>}
            </span>
            <span className="text-section font-semibold">{card.name}</span>
            <span id={descId} className="flex flex-1 flex-col gap-2 text-body text-muted">
              <span>{card.available ? card.description : `${TUTORIAL_LOCKED}.`}</span>
              <span className="mt-auto text-caption font-semibold text-ink">Recompensa: {TUTORIAL_REWARD_XP} de EXP</span>
            </span>
          </label>
        );
      })}
    </fieldset>
  );
}

function LockIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="5" y="11" width="14" height="10" rx="2" />
      <path d="M8 11V7a4 4 0 0 1 8 0v4" />
    </svg>
  );
}

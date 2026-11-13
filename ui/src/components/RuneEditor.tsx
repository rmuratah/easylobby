import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { assetUrl } from '../agent/assets';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { AssetKind } from '../bindings/AssetKind';
import type { Perk } from '../bindings/Perk';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { RunePage } from '../bindings/RunePage';
import type { RunePages } from '../bindings/RunePages';
import type { RuneStyle } from '../bindings/RuneStyle';
import type { SlotRunesInput } from '../bindings/SlotRunesInput';
import { Select } from './base';
import { cx } from './base/cx';
import {
  allowedSubStyles, draftFromPage, draftProblem, draftToInput, findStyle, newDraft, newPageName, primarySlots, slotRunes, statSlots, subSlots,
  withPrimary, withPrimaryPerk, withStatPerk, withSubPerk, withSubStyle, type RuneDraft,
} from './runePage';

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';

export function RuneIcon({ kind, id, size = 24, className }: { kind: AssetKind; id: number; size?: number; className?: string }) {
  // O fundo fica no lugar do ícone enquanto ele carrega ou quando falta.
  return (
    <span aria-hidden="true" className={cx('inline-block shrink-0 overflow-hidden rounded-full bg-raised', className)}
      style={{ width: size, height: size }}>
      {id ? <img src={assetUrl(kind, id)} alt="" width={size} height={size}
        onError={(event) => { event.currentTarget.style.visibility = 'hidden'; }} /> : null}
    </span>
  );
}

/**
 * Caixa de informação que aparece ao lado de um ícone com o mouse em cima
 * ou com o foco do teclado (#420), como a do client oficial: título em
 * caixa alta e a descrição completa da runa. Fica num portal, para não ser
 * cortada pela rolagem do pop-up, e se ajusta para caber na janela.
 */
function Tooltip({ anchor, id, title, children }: { anchor: HTMLElement; id: string; title: string; children?: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const box = ref.current?.getBoundingClientRect();
    const target = anchor.getBoundingClientRect();
    if (!box) return;
    const gap = 10;
    const width = window.innerWidth;
    const height = window.innerHeight;
    // À direita do ícone; sem espaço, à esquerda; sem nenhum dos dois, embaixo.
    let left = target.right + gap;
    let top = target.top;
    if (left + box.width > width - gap) left = target.left - gap - box.width;
    if (left < gap) {
      left = Math.min(Math.max(gap, target.left + target.width / 2 - box.width / 2), width - gap - box.width);
      top = target.bottom + gap;
    }
    top = Math.max(gap, Math.min(top, height - gap - box.height));
    setPosition({ left, top });
  }, [anchor]);
  return createPortal(
    <div ref={ref} id={id} role="tooltip"
      className="pointer-events-none fixed z-[60] w-[20rem] max-w-[calc(100vw-1.25rem)] rounded-control border border-line bg-surface px-4 py-3 text-ink shadow-lift"
      style={position ? { left: position.left, top: position.top } : { left: 0, top: 0, visibility: 'hidden' }}>
      <p className="text-body font-bold uppercase tracking-[0.04em]">{title}</p>
      {children}
    </div>,
    document.body,
  );
}

/** Mostra a [`Tooltip`] enquanto o mouse ou o foco estão no elemento. */
function useTooltip() {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const id = useId();
  const handlers = {
    onMouseEnter: (event: { currentTarget: HTMLElement }) => setAnchor(event.currentTarget),
    onMouseLeave: () => setAnchor(null),
    onFocus: (event: { currentTarget: HTMLElement }) => setAnchor(event.currentTarget),
    onBlur: () => setAnchor(null),
  };
  return { anchor, id, handlers };
}

/** Descrição da runa para a caixa: a completa, ou a curta quando o LoL não mandou a completa. */
function perkDescription(perk: Perk | undefined): string {
  return perk?.longDesc || perk?.shortDesc || '';
}

const runeButton = cx('flex items-center justify-center rounded-full p-0.5 transition-colors hover:bg-raised',
  'aria-pressed:bg-accent-soft aria-pressed:ring-2 aria-pressed:ring-accent', focusRing);

/** Linha de runas em grade: cada ícone no centro de uma parte igual da largura. */
const runeRow = 'grid justify-items-center gap-1';
const columns = (count: number) => ({ gridTemplateColumns: `repeat(${Math.max(count, 1)}, minmax(0, 1fr))` });

function PerkButton({ perk, id, size, selected, dimmed, disabled, onChoose }: {
  perk: Perk | undefined; id: number; size: number; selected: boolean; dimmed: boolean; disabled?: boolean; onChoose: () => void;
}) {
  const tooltip = useTooltip();
  const name = perk?.name || `Runa ${id}`;
  const description = perkDescription(perk);
  return (
    <>
      <button type="button" aria-pressed={selected} aria-label={name} aria-describedby={tooltip.anchor ? tooltip.id : undefined}
        disabled={disabled} onClick={onChoose} className={runeButton} {...tooltip.handlers}>
        <RuneIcon kind="perk" id={id} size={size} className={cx(dimmed && 'opacity-50 grayscale')} />
      </button>
      {tooltip.anchor && (
        <Tooltip anchor={tooltip.anchor} id={tooltip.id} title={name}>
          {description && <p className="mt-2 whitespace-pre-line text-body text-muted">{description}</p>}
        </Tooltip>
      )}
    </>
  );
}

function StyleButton({ style, selected, disabled, onChoose }: { style: RuneStyle; selected: boolean; disabled?: boolean; onChoose: () => void }) {
  const tooltip = useTooltip();
  return (
    <>
      <button type="button" aria-pressed={selected} aria-label={style.name} disabled={disabled}
        aria-describedby={tooltip.anchor ? tooltip.id : undefined} onClick={onChoose} className={runeButton} {...tooltip.handlers}>
        <RuneIcon kind="perkStyle" id={style.id} size={34} className={cx(!selected && 'opacity-60')} />
      </button>
      {tooltip.anchor && <Tooltip anchor={tooltip.anchor} id={tooltip.id} title={style.name} />}
    </>
  );
}

/**
 * As árvores e as runas em grade (#331, #401): a primária à esquerda e a
 * secundária com os fragmentos à direita, como no client oficial. Cada
 * linha divide a largura em partes iguais, e as linhas se espalham na
 * altura, para os ícones formarem uma grade e as colunas, dois retângulos.
 */
function RuneGrid({ catalog, draft, disabled, onChange }: {
  catalog: RuneCatalog; draft: RuneDraft; disabled: boolean; onChange: (draft: RuneDraft) => void;
}) {
  const primary = findStyle(catalog, draft.primaryStyleId);
  const sub = findStyle(catalog, draft.subStyleId);
  const perk = (id: number) => catalog.perks.find(p => p.id === id);

  function row(perks: number[], label: string, selected: (id: number) => boolean, choose: (id: number) => void, size: number) {
    const anySelected = perks.some(selected);
    return (
      <div role="group" aria-label={label} className={runeRow} style={columns(perks.length)}>
        {perks.map(id => (
          <PerkButton key={id} id={id} perk={perk(id)} size={size} selected={selected(id)} dimmed={anySelected && !selected(id)}
            disabled={disabled} onChoose={() => choose(id)} />
        ))}
      </div>
    );
  }
  function styles(options: RuneStyle[], selected: number, label: string, choose: (id: number) => void) {
    return (
      <div role="group" aria-label={label} className={runeRow} style={columns(options.length)}>
        {options.map(style => (
          <StyleButton key={style.id} style={style} selected={style.id === selected} disabled={disabled} onChoose={() => choose(style.id)} />
        ))}
      </div>
    );
  }

  return (
    <div className="grid flex-1 grid-cols-2">
      <div className="flex min-w-0 flex-col justify-between gap-2 pr-4">
        {styles(catalog.styles, draft.primaryStyleId, 'Árvore primária', id => onChange(withPrimary(catalog, draft, id)))}
        {primarySlots(primary).map((slot, index) => (
          <div key={`p${index}`}>{row(slot.perks, slot.kind === 'keystone' ? 'Runa principal' : `Primária, linha ${index}`,
            id => draft.primary[index] === id, id => onChange(withPrimaryPerk(draft, index, id)), slot.kind === 'keystone' ? 60 : 46)}</div>
        ))}
      </div>
      <div className="flex min-w-0 flex-col justify-between gap-2 border-l border-line pl-4">
        {styles(allowedSubStyles(catalog, primary), draft.subStyleId, 'Árvore secundária', id => onChange(withSubStyle(draft, id)))}
        {subSlots(sub).map((slot, index) => (
          <div key={`s${index}`}>{row(slot.perks, `Secundária, linha ${index + 1}`,
            id => draft.sub.some(pick => pick.row === index && pick.perkId === id), id => onChange(withSubPerk(draft, index, id)), 42)}</div>
        ))}
        {statSlots(primary).map((slot, index) => (
          <div key={`f${index}`}>{row(slot.perks, slot.label || `Fragmento ${index + 1}`,
            id => draft.stats[index] === id, id => onChange(withStatPerk(draft, index, id)), 30)}</div>
        ))}
      </div>
    </div>
  );
}

/** Se o rascunho mudou alguma coisa da página (nome, árvores ou runas). */
function changedFrom(catalog: RuneCatalog, draft: RuneDraft, page: RunePage): boolean {
  const a = draftToInput(draft);
  const b = draftToInput(draftFromPage(catalog, page));
  return a.name !== page.name.trim() || a.primaryStyleId !== b.primaryStyleId || a.subStyleId !== b.subStyleId
    || a.selectedPerkIds.join(',') !== b.selectedPerkIds.join(',');
}

/** Onde o Salvar grava: as runas do slot, uma página existente ou uma página nova. */
type Target = { kind: 'slot' } | { kind: 'page'; id: number } | { kind: 'new' };

export type SlotRunes = {
  /** Runas que o slot já tem, ou `null` para começar numa página vazia. */
  current: { perkIds: number[]; styleId: number; subStyleId: number } | null;
  onSave: (runes: SlotRunesInput) => void;
};

type EditorProps = {
  catalog: RuneCatalog;
  runePages: RunePages | null;
  send: AgentClient['send'];
  /**
   * Editor das runas de um slot do Jogo Dinâmico (#331). Sem isto, o editor
   * mexe na página em uso, como na seleção de campeões.
   */
  slot?: SlotRunes;
  className?: string;
};

/**
 * Editor de páginas de runas, o mesmo no pop-up do Jogo Dinâmico e na
 * seleção de campeões de todos os modos (#420). Como no client oficial, o
 * cabeçalho tem o lápis para renomear, o nome com a escolha de página, o
 * Salvar, o + para criar uma página e a lixeira para excluir a página.
 *
 * - Na seleção de campeões, escolher uma página a aplica (como no client
 *   oficial) e o editor mostra a página em uso; o Salvar grava nela.
 * - No slot do Jogo Dinâmico, escolher uma página só a carrega no editor
 *   (#417); o Salvar grava as runas no slot e, se uma página foi carregada
 *   ou criada, grava a página também.
 */
export function RunePageEditor({ catalog, runePages, send, slot, className }: EditorProps) {
  // No slot, as páginas recomendadas (temporárias) ficam de fora: não dá para gravar nelas.
  const pages = (runePages?.pages ?? []).filter(p => !slot || !p.isTemporary);
  const command = useCommand(send);
  const [target, setTarget] = useState<Target>(() => slot ? { kind: 'slot' } : { kind: 'page', id: runePages?.currentPageId ?? pages[0]?.id ?? 0 });
  // Na seleção, o editor segue a página em uso: aplicar outra (aqui, no
  // atalho da seleção ou no LoL) a traz para o editor.
  const followed = !slot && target.kind === 'page' ? (runePages?.currentPageId ?? target.id) : null;
  const effective: Target = followed !== null ? { kind: 'page', id: followed } : target;
  const page = effective.kind === 'page' ? pages.find(p => p.id === effective.id) : undefined;
  const sourceKey = effective.kind === 'page' ? `p:${page ? JSON.stringify(page) : effective.id}` : effective.kind;
  const [draft, setDraftState] = useState<{ key: string; value: RuneDraft }>(() => ({ key: sourceKey, value: initialDraft() }));
  const [renaming, setRenaming] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const pageSelectId = useId();
  const nameRef = useRef<HTMLInputElement>(null);
  const nameBefore = useRef('');

  function initialDraft(): RuneDraft {
    if (page) return draftFromPage(catalog, page);
    if (slot?.current && slot.current.perkIds.length) {
      return draftFromPage(catalog, { name: '', primaryStyleId: slot.current.styleId, subStyleId: slot.current.subStyleId, selectedPerkIds: slot.current.perkIds });
    }
    return newDraft(catalog, '');
  }

  // A página mudou por fora (salva, aplicada no LoL): o editor volta para ela,
  // menos com um comando no caminho.
  if (draft.key !== sourceKey && effective.kind === 'page' && !command.pending) {
    setDraftState({ key: sourceKey, value: initialDraft() });
  }
  const value = draft.value;
  const setDraft = (next: RuneDraft) => setDraftState({ key: draft.key, value: next });

  useEffect(() => { if (renaming) { nameRef.current?.focus(); nameRef.current?.select(); } }, [renaming]);

  const busy = command.pending;
  const editable = effective.kind === 'new' || effective.kind === 'slot' || !!page?.isEditable;
  const named = effective.kind !== 'slot';
  const problem = draftProblem({ ...value, name: named ? value.name : 'Slot' });
  const dirty = effective.kind === 'new' || (page ? changedFrom(catalog, value, page) : false);
  const createBlocked = runePages && !runePages.canCreatePage
    ? 'Todas as páginas estão ocupadas. Exclua uma para criar outra.' : null;
  const canDelete = effective.kind === 'page' && !!page?.isDeletable;

  let hint: string | null = null;
  if (problem) hint = problem;
  else if (effective.kind === 'page' && page && !page.isEditable) hint = 'Esta página não pode ser editada. Use o + para salvar uma cópia.';
  else if (effective.kind === 'page' && !page) hint = 'Escolha uma página ou crie uma com o +.';
  const saveDisabled = busy || !!problem || !editable || (effective.kind === 'page' && (!page || (!slot && !dirty)));

  function choosePage(raw: string) {
    setRenaming(false);
    setConfirmDelete(false);
    if (raw === 'slot') { setTarget({ kind: 'slot' }); setDraftState({ key: 'slot', value: initialSlotDraft() }); return; }
    if (raw === 'new') return;
    const id = Number(raw);
    const chosen = pages.find(p => p.id === id);
    if (!chosen) return;
    if (slot) {
      // Só carrega no editor: nada vai para o LoL sem o Salvar (#417).
      setTarget({ kind: 'page', id });
      setDraftState({ key: `p:${JSON.stringify(chosen)}`, value: draftFromPage(catalog, chosen) });
      return;
    }
    setTarget({ kind: 'page', id });
    if (id !== runePages?.currentPageId) {
      void command.run({ type: 'selectRunePage', pageId: id }, { pending: 'Aplicando página…', failure: 'aplicar a página' });
    }
  }

  function initialSlotDraft(): RuneDraft {
    if (slot?.current && slot.current.perkIds.length) {
      return draftFromPage(catalog, { name: '', primaryStyleId: slot.current.styleId, subStyleId: slot.current.subStyleId, selectedPerkIds: slot.current.perkIds });
    }
    return newDraft(catalog, '');
  }

  function create() {
    setConfirmDelete(false);
    // Começa com as runas que estão no editor, com um nome novo.
    const complete = !draftProblem({ ...value, name: 'x' });
    setTarget({ kind: 'new' });
    setDraftState({ key: 'new', value: { ...(complete ? value : newDraft(catalog, '')), name: newPageName(pages) } });
    setRenaming(true);
  }

  function cancelNew() {
    setRenaming(false);
    if (slot) { setTarget({ kind: 'slot' }); setDraftState({ key: 'slot', value: initialSlotDraft() }); return; }
    setTarget({ kind: 'page', id: runePages?.currentPageId ?? pages[0]?.id ?? 0 });
  }

  async function remove() {
    if (effective.kind !== 'page' || !page) return;
    setConfirmDelete(false);
    const done = await command.run({ type: 'deleteRunePage', pageId: page.id },
      { pending: 'Excluindo página…', success: 'Página excluída.', failure: 'excluir a página' });
    if (!done) return;
    if (slot) { setTarget({ kind: 'slot' }); setDraftState({ key: 'slot', value: value }); return; }
    setTarget({ kind: 'page', id: 0 });
  }

  async function save() {
    if (saveDisabled) return;
    setRenaming(false);
    const input = draftToInput(value);
    if (effective.kind === 'new') {
      const done = await command.run({ type: 'createRunePage', page: input },
        { pending: 'Criando página…', success: 'Página criada.', failure: 'criar a página' });
      if (!done) return;
      // A página criada fica em uso; na seleção, o editor passa a mostrá-la.
      if (!slot) setTarget({ kind: 'page', id: 0 });
    } else if (effective.kind === 'page' && page && dirty) {
      const done = await command.run({ type: 'updateRunePage', pageId: page.id, page: input },
        { pending: 'Salvando página…', success: 'Página salva.', failure: 'salvar a página' });
      if (!done) return;
    }
    slot?.onSave(slotRunes(value));
  }

  const title = effective.kind === 'slot' ? 'Runas do slot' : value.name || 'Sem nome';
  const iconButton = cx('flex size-9 shrink-0 items-center justify-center rounded-full border border-line bg-surface text-ink transition-colors',
    'enabled:hover:border-accent enabled:hover:text-accent-ink disabled:cursor-not-allowed disabled:opacity-40', focusRing);

  return (
    <form aria-label="Editar runas" className={cx('flex min-h-[26rem] flex-1 flex-col gap-3', className)}
      onSubmit={event => { event.preventDefault(); void save(); }}>
      <div className="flex flex-wrap items-center gap-2">
        <button type="button" className={iconButton} aria-label="Renomear página" title={named ? 'Renomear página' : 'Escolha ou crie uma página para dar nome'}
          disabled={!named || !editable || busy} aria-pressed={renaming}
          // Sem tirar o foco do nome: o clique no lápis fecha a edição em vez de reabrir.
          onMouseDown={event => event.preventDefault()} onClick={() => {
            nameBefore.current = value.name;
            setRenaming(!renaming);
          }}>
          <PencilIcon />
        </button>
        <div className="flex min-w-0 flex-1 basis-56 items-center gap-2">
          {renaming && named
            ? (
              <input ref={nameRef} aria-label="Nome da página" value={value.name} maxLength={64} disabled={busy}
                onChange={event => setDraft({ ...value, name: event.target.value })}
                onKeyDown={event => {
                  if (event.key === 'Enter') { event.preventDefault(); setRenaming(false); }
                  if (event.key === 'Escape') {
                    // Esc desfaz o nome sem fechar o pop-up.
                    event.preventDefault();
                    event.stopPropagation();
                    setDraft({ ...value, name: nameBefore.current });
                    setRenaming(false);
                  }
                }}
                onBlur={() => setRenaming(false)}
                className={cx('h-9 min-w-0 flex-1 rounded-control border border-accent bg-surface px-3 text-body font-semibold text-ink', focusRing)} />
            )
            : (
              <>
                <label htmlFor={pageSelectId} className="sr-only">Página de runas</label>
                <Select id={pageSelectId} size="sm" className="h-9 min-w-0 flex-1 text-body font-semibold"
                  disabled={busy || (!slot && pages.length === 0 && effective.kind !== 'new')}
                  value={effective.kind === 'page' ? (page ? String(effective.id) : null) : effective.kind}
                  placeholder="Escolha uma página"
                  options={[
                    ...(slot ? [{ value: 'slot', label: 'Runas do slot' }] : []),
                    ...(effective.kind === 'new' ? [{ value: 'new', label: `${title} (nova)` }] : []),
                    ...pages.map(p => ({
                      value: String(p.id),
                      label: effective.kind === 'page' && p.id === effective.id ? title : p.name,
                      hint: p.isTemporary ? 'Recomendada' : undefined,
                    })),
                  ]}
                  onChange={choosePage} />
              </>
            )}
        </div>
        <button type="submit" disabled={saveDisabled}
          className={cx('inline-flex h-9 shrink-0 items-center rounded-full bg-accent px-4 text-caption font-semibold uppercase tracking-[0.06em] text-on-accent disabled:cursor-not-allowed disabled:bg-raised disabled:text-muted', focusRing)}>
          Salvar
        </button>
        <button type="button" className={iconButton} aria-label="Criar página" title={createBlocked ?? 'Criar página'}
          disabled={busy || !!createBlocked || !runePages || effective.kind === 'new'} onClick={create}>
          <PlusIcon />
        </button>
        {effective.kind === 'new'
          ? (
            <button type="button" className={iconButton} aria-label="Descartar página nova" title="Descartar página nova" disabled={busy} onClick={cancelNew}>
              <CloseIcon />
            </button>
          )
          : (
            <button type="button" className={iconButton} aria-label="Excluir página"
              title={canDelete ? 'Excluir página' : 'Escolha uma página sua para excluir'}
              disabled={busy || !canDelete} onClick={() => setConfirmDelete(true)}>
              <TrashIcon />
            </button>
          )}
      </div>
      {confirmDelete && page && (
        <div role="alertdialog" aria-label="Excluir página" className="flex flex-wrap items-center gap-2 rounded-control border border-line bg-raised px-3 py-2">
          <span className="min-w-0 flex-1 text-caption font-semibold text-warn">Excluir “{page.name}”? Não dá para desfazer.</span>
          <button type="button" onClick={() => { void remove(); }} disabled={busy}
            className={cx('inline-flex h-8 items-center rounded-full bg-danger px-3 text-caption font-semibold text-on-accent', focusRing)}>
            Excluir
          </button>
          <button type="button" onClick={() => setConfirmDelete(false)}
            className={cx('inline-flex h-8 items-center rounded-full border border-line px-3 text-caption font-semibold text-ink hover:bg-surface', focusRing)}>
            Manter
          </button>
        </div>
      )}
      <RuneGrid catalog={catalog} draft={value} disabled={busy || !editable} onChange={setDraft} />
      <div className="flex min-h-6 items-center gap-2 border-t border-line pt-2">
        {command.feedback.status !== 'idle'
          ? <p role={command.feedback.status === 'error' ? 'alert' : 'status'}
            className={cx('min-w-0 text-caption', command.feedback.status === 'error' ? 'text-danger' : 'text-muted')}>{command.feedback.message}</p>
          : <p className={cx('min-w-0 text-caption', hint ? 'text-warn' : 'text-muted')}>{hint ?? 'Passe o mouse numa runa para ver o que ela faz.'}</p>}
      </div>
    </form>
  );
}

export function PencilIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 20h4L19 9l-4-4L4 16v4z" /><path d="M13.5 6.5l4 4" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

function TrashIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round">
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  );
}

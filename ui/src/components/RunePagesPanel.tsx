import { useEffect, useRef, useState, type ReactNode } from 'react';
import { assetUrl } from '../agent/assets';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { AssetKind } from '../bindings/AssetKind';
import type { Command } from '../bindings/Command';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { RunePage } from '../bindings/RunePage';
import type { RunePages } from '../bindings/RunePages';
import type { RuneSlot } from '../bindings/RuneSlot';
import {
  allowedSubStyles, draftFromPage, draftProblem, draftToInput, findStyle, newDraft, primarySlots, statSlots, subSlots,
  withPrimary, withPrimaryPerk, withStatPerk, withSubPerk, withSubStyle, type RuneDraft,
} from './runePage';
import { Button, Card, TextField } from './base';
import { cx } from './base/cx';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type RuneCommand = Extract<Command, { type: 'selectRunePage' | 'createRunePage' | 'updateRunePage' | 'deleteRunePage' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
type Editing = { mode: 'create' } | { mode: 'edit'; pageId: number };

// Escolhas do editor: o selecionado ganha a cor de destaque; os outros da mesma linha ficam apagados.
const choiceClass = cx(
  'flex items-center gap-2 rounded-control border border-line bg-surface px-2 py-1.5 text-left text-body text-ink',
  'hover:bg-raised aria-pressed:border-accent aria-pressed:bg-accent-soft aria-pressed:text-accent-ink',
  'disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
);
const legendClass = 'mb-1 text-caption font-semibold uppercase tracking-[0.06em] text-muted';

const pending: Record<RuneCommand['type'], string> = {
  selectRunePage: 'Aplicando página…',
  createRunePage: 'Criando página…',
  updateRunePage: 'Salvando página…',
  deleteRunePage: 'Apagando página…',
};
const success: Record<RuneCommand['type'], string> = {
  selectRunePage: 'Página aplicada.',
  createRunePage: 'Página criada e aplicada.',
  updateRunePage: 'Página salva.',
  deleteRunePage: 'Página apagada.',
};
const failure: Record<RuneCommand['type'], string> = {
  selectRunePage: 'aplicar a página',
  createRunePage: 'criar a página',
  updateRunePage: 'salvar a página',
  deleteRunePage: 'apagar a página',
};

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

function Shell({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <Card as="section" aria-label="Páginas de runas" className={cx('flex w-full max-w-[56rem] flex-col gap-4 p-5 text-ink', className)}>
      {children}
    </Card>
  );
}

const heading = <h2 className="text-title font-semibold tracking-[-0.015em]">Páginas de runas</h2>;

/**
 * Lista, aplica, cria, edita e apaga páginas de runas (#26). Só mostra o
 * estado confirmado pelo agente; o rascunho do editor é a única coisa
 * local, e some quando a conexão com o LoL muda.
 */
export function RunePagesPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return <Shell>{heading}<p className="text-body text-muted">Aguarde a reconexão. As páginas serão confirmadas novamente pelo agente.</p></Shell>;
  }
  if (!state.runePages) {
    return <Shell>{heading}<p className="text-body text-muted">{state.summoner ? 'Carregando páginas de runas…' : 'Aguarde o login no League of Legends.'}</p></Shell>;
  }
  return (
    <RunePagesContent key={`${lcu.session}:${state.summoner?.puuid ?? ''}`}
      runePages={state.runePages} catalog={state.runeCatalog} send={send} />
  );
}

function RunePagesContent({ runePages, catalog, send }: { runePages: RunePages; catalog: RuneCatalog | null; send: AgentClient['send'] }) {
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const [editing, setEditing] = useState<Editing | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<number | null>(null);
  const inFlight = useRef(false);
  const mounted = useRef(false);
  // Quem abriu o editor recebe o foco de volta quando ele fecha.
  const opener = useRef<HTMLElement | null>(null);
  const createButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  const busy = feedback.status === 'pending';
  const { pages, currentPageId, canCreatePage } = runePages;
  const editedPage = editing?.mode === 'edit' ? pages.find((page) => page.id === editing.pageId) : undefined;
  // A página em edição pode sumir por fora (apagada no LoL); aí o editor fecha.
  const openEditor = editing && catalog && (editing.mode === 'create' || editedPage?.isEditable) ? editing : null;
  const editorOpen = !!openEditor;
  useEffect(() => {
    if (editorOpen) return;
    const target = opener.current?.isConnected ? opener.current : null;
    opener.current = null;
    if (target && document.activeElement === document.body) (target.matches(':disabled') ? createButton.current : target)?.focus();
  }, [editorOpen]);

  function startEditing(next: Editing) {
    opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setConfirmDelete(null);
    setEditing(next);
  }

  async function execute(command: RuneCommand): Promise<boolean> {
    if (inFlight.current) return false;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: pending[command.type] });
    try {
      const result = await send(command);
      if (!mounted.current) return false;
      if (result.status === 'ok') {
        setFeedback({ status: 'success', message: success[command.type] });
        return true;
      }
      setFeedback({ status: 'error', message: result.code === 'notConnected'
        ? 'O League of Legends desconectou. Aguarde a reconexão e confira as páginas antes de tentar novamente.'
        : `Não foi possível ${failure[command.type]}: ${result.message}` });
    } catch {
      if (mounted.current) {
        setFeedback({ status: 'error', message: 'Não foi possível confirmar o comando. Confira a conexão e as páginas antes de tentar novamente.' });
      }
    } finally {
      inFlight.current = false;
    }
    return false;
  }

  async function remove(pageId: number) {
    setConfirmDelete(null);
    if (await execute({ type: 'deleteRunePage', pageId }) && editing?.mode === 'edit' && editing.pageId === pageId) {
      setEditing(null);
    }
  }

  async function save(draft: RuneDraft) {
    const page = draftToInput(draft);
    const done = editing?.mode === 'edit'
      ? await execute({ type: 'updateRunePage', pageId: editing.pageId, page })
      : await execute({ type: 'createRunePage', page });
    if (done) setEditing(null);
  }

  const createHint = !catalog ? 'Carregando as árvores de runas…'
    : !canCreatePage ? 'Todas as páginas estão ocupadas. Apague uma para criar outra.'
    : null;

  return (
    <Shell>
      <div className="flex flex-wrap items-center justify-between gap-3">
        {heading}
        {!openEditor && (
          <Button ref={createButton} variant="primary" size="sm" disabled={!!createHint || busy}
            onClick={() => startEditing({ mode: 'create' })}>Nova página</Button>
        )}
      </div>
      {!openEditor && createHint && <p className="-mt-2 text-caption text-muted">{createHint}</p>}
      {pages.length === 0
        ? <p className="text-body text-muted">Nenhuma página de runas.</p>
        : (
          <ul aria-label="Páginas" className="grid grid-cols-[repeat(auto-fill,minmax(17rem,1fr))] gap-2">
            {pages.map((page) => (
              <PageItem key={page.id} page={page} catalog={catalog} current={page.id === currentPageId} busy={busy}
                editing={openEditor?.mode === 'edit' && openEditor.pageId === page.id}
                confirming={confirmDelete === page.id}
                onSelect={() => { void execute({ type: 'selectRunePage', pageId: page.id }); }}
                onEdit={() => startEditing({ mode: 'edit', pageId: page.id })}
                onDelete={() => setConfirmDelete(page.id)}
                onConfirmDelete={() => { void remove(page.id); }}
                onCancelDelete={() => setConfirmDelete(null)} />
            ))}
          </ul>
        )}
      {feedback.message && (
        <p role={feedback.status === 'error' ? 'alert' : 'status'}
          className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>
      )}
      {openEditor && catalog && (
        <RunePageEditor key={openEditor.mode === 'edit' ? openEditor.pageId : 'nova'} catalog={catalog} busy={busy}
          initial={editedPage ? draftFromPage(catalog, editedPage) : newDraft(catalog, `Página ${pages.length + 1}`)}
          title={editedPage ? `Editar ${editedPage.name}` : 'Nova página'}
          saveLabel={editedPage ? 'Salvar página' : 'Criar e aplicar'}
          onSave={(draft) => { void save(draft); }} onCancel={() => setEditing(null)} />
      )}
    </Shell>
  );
}

type PageItemProps = {
  page: RunePage; catalog: RuneCatalog | null; current: boolean; busy: boolean; editing: boolean; confirming: boolean;
  onSelect: () => void; onEdit: () => void; onDelete: () => void; onConfirmDelete: () => void; onCancelDelete: () => void;
};

function PageItem({ page, catalog, current, busy, editing, confirming, onSelect, onEdit, onDelete, onConfirmDelete, onCancelDelete }: PageItemProps) {
  const primary = catalog && findStyle(catalog, page.primaryStyleId);
  const sub = catalog && findStyle(catalog, page.subStyleId);
  const keystone = catalog?.perks.find((perk) => perk.id === page.selectedPerkIds[0]);
  const summary = [keystone?.name, primary?.name, sub?.name].filter(Boolean).join(' · ');
  const tags = [current && 'Em uso', page.isTemporary && 'Temporária', !page.isValid && 'Incompleta'].filter(Boolean).join(' · ');
  return (
    <li aria-label={page.name} aria-current={current ? 'true' : undefined}
      className={cx('flex flex-col gap-3 rounded-card border p-3',
        current ? 'border-accent bg-accent-soft' : 'border-line bg-surface', editing && 'outline-2 outline-offset-2 outline-accent')}>
      <div className="flex min-w-0 items-center gap-3">
        <span className="relative shrink-0">
          <RuneIcon kind="perk" id={page.selectedPerkIds[0] ?? 0} size={40} />
          <RuneIcon kind="perkStyle" id={page.subStyleId} size={20}
            className="absolute -bottom-1 -right-1 border-2 border-surface" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="break-words font-semibold">{page.name}</span>
          <span className="text-caption text-muted">{summary || 'Sem runas'}</span>
          {tags && <span className={cx('text-caption font-semibold', page.isValid ? 'text-accent-ink' : 'text-warn')}>{tags}</span>}
        </div>
      </div>
      {confirming
        ? (
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-caption font-semibold text-warn">Apagar esta página?</span>
            <Button variant="danger" size="sm" disabled={busy} onClick={onConfirmDelete}>Confirmar exclusão</Button>
            <Button size="sm" onClick={onCancelDelete}>Manter</Button>
          </div>
        )
        : (
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant={current ? 'ghost' : 'secondary'} aria-label={`Usar ${page.name}`}
              disabled={current || busy} onClick={onSelect}>Usar</Button>
            {page.isEditable && (
              <Button size="sm" variant="ghost" aria-label={`Editar ${page.name}`} disabled={!catalog || editing || busy} onClick={onEdit}>Editar</Button>
            )}
            {page.isDeletable && (
              <Button size="sm" variant="ghost" aria-label={`Apagar ${page.name}`} disabled={busy} onClick={onDelete}>Apagar</Button>
            )}
          </div>
        )}
    </li>
  );
}

type EditorProps = {
  catalog: RuneCatalog; initial: RuneDraft; title: string; saveLabel: string; busy: boolean;
  onSave: (draft: RuneDraft) => void; onCancel: () => void;
};

function RunePageEditor({ catalog, initial, title, saveLabel, busy, onSave, onCancel }: EditorProps) {
  const [draft, setDraft] = useState(initial);
  const nameRef = useRef<HTMLInputElement>(null);
  // O editor abre com o foco no nome, para quem chegou pelo teclado continuar dali.
  useEffect(() => { nameRef.current?.focus(); }, []);
  const primary = findStyle(catalog, draft.primaryStyleId);
  const sub = findStyle(catalog, draft.subStyleId);
  const problem = draftProblem(draft);
  const perk = (id: number) => catalog.perks.find((p) => p.id === id);

  function row(slot: RuneSlot, legend: string, selected: (perkId: number) => boolean, choose: (perkId: number) => void, key: string, size: number) {
    const anySelected = slot.perks.some(selected);
    return (
      <fieldset key={key} className="flex min-w-0 flex-col">
        <legend className={legendClass}>{legend}</legend>
        <div className="flex flex-wrap gap-1.5">
          {slot.perks.map((perkId) => {
            const info = perk(perkId);
            const on = selected(perkId);
            return (
              <button key={perkId} type="button" className={choiceClass} aria-pressed={on}
                title={info?.shortDesc || undefined} disabled={busy} onClick={() => choose(perkId)}>
                <RuneIcon kind="perk" id={perkId} size={size} className={cx(anySelected && !on && 'opacity-60 grayscale')} />
                {info?.name ?? `Runa ${perkId}`}
              </button>
            );
          })}
        </div>
      </fieldset>
    );
  }

  function styles(legend: string, options: typeof catalog.styles, selected: number, choose: (id: number) => void) {
    return (
      <fieldset className="flex min-w-0 flex-col">
        <legend className="mb-1 text-section font-semibold">{legend}</legend>
        <div className="flex flex-wrap gap-1.5">
          {options.map((style) => (
            <button key={style.id} type="button" className={choiceClass} aria-pressed={style.id === selected}
              disabled={busy} onClick={() => choose(style.id)}>
              <RuneIcon kind="perkStyle" id={style.id} size={24} />
              {style.name}
            </button>
          ))}
        </div>
      </fieldset>
    );
  }

  return (
    <form aria-label={title} className="flex flex-col gap-4 rounded-card border border-accent bg-surface card p-4"
      onKeyDown={(event) => {
        // Esc cancela a edição, menos quando outro controle já tratou a tecla.
        if (event.key === 'Escape' && !event.defaultPrevented) { event.preventDefault(); onCancel(); }
      }}
      onSubmit={(event) => { event.preventDefault(); if (!problem && !busy) onSave(draft); }}>
      <div className="flex flex-wrap items-end justify-between gap-3">
        <h3 className="text-section font-semibold">{title}</h3>
        <TextField ref={nameRef} label="Nome da página" className="w-full max-w-[20rem]" value={draft.name} maxLength={64}
          disabled={busy} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
      </div>
      <div className="rune-editor">
        <div className="flex min-w-0 flex-col gap-3">
          {styles('Árvore primária', catalog.styles, draft.primaryStyleId,
            (id) => setDraft(withPrimary(catalog, draft, id)))}
          {primarySlots(primary).map((slot, index) => row(slot,
            slot.label || (slot.kind === 'keystone' ? 'Runa principal' : `Linha ${index}`),
            (perkId) => draft.primary[index] === perkId,
            (perkId) => setDraft(withPrimaryPerk(draft, index, perkId)), `p${index}`, slot.kind === 'keystone' ? 36 : 28))}
        </div>
        <div className="flex min-w-0 flex-col gap-3">
          {styles('Árvore secundária', allowedSubStyles(catalog, primary), draft.subStyleId,
            (id) => setDraft(withSubStyle(draft, id)))}
          {subSlots(sub).map((slot, index) => row(slot,
            slot.label || `Secundária, linha ${index + 1}`,
            (perkId) => draft.sub.some((pick) => pick.row === index && pick.perkId === perkId),
            (perkId) => setDraft(withSubPerk(draft, index, perkId)), `s${index}`, 28))}
          {statSlots(primary).map((slot, index) => row(slot,
            slot.label || `Fragmento ${index + 1}`,
            (perkId) => draft.stats[index] === perkId,
            (perkId) => setDraft(withStatPerk(draft, index, perkId)), `f${index}`, 20))}
        </div>
      </div>
      {/* Fica preso no rodapé da janela (descontando o respiro da área), para salvar sem rolar até o fim. */}
      <div className="sticky -bottom-6 -mx-4 -mb-4 flex flex-wrap items-center gap-3 rounded-b-card border-t border-line bg-surface px-4 py-3">
        <Button type="submit" variant="primary" loading={busy} disabled={!!problem}>{saveLabel}</Button>
        <Button onClick={onCancel}>Cancelar</Button>
        {problem
          ? <p className="text-caption text-warn">{problem}</p>
          : <p className="text-caption text-muted">Enter salva · Esc cancela</p>}
      </div>
    </form>
  );
}

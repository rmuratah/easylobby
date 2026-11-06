import { useEffect, useRef, useState } from 'react';
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

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type RuneCommand = Extract<Command, { type: 'selectRunePage' | 'createRunePage' | 'updateRunePage' | 'deleteRunePage' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
type Editing = { mode: 'create' } | { mode: 'edit'; pageId: number };

const buttonClass = 'rounded border border-sky-400 px-3 py-1 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-4';
const choiceClass = 'flex items-center gap-2 rounded border px-2 py-1 text-left text-sm aria-pressed:border-sky-400 aria-pressed:bg-sky-950 border-slate-700 disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2';

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

function Icon({ kind, id, size = 24 }: { kind: AssetKind; id: number; size?: number }) {
  if (!id) return null;
  return (
    <img src={assetUrl(kind, id)} alt="" width={size} height={size} className="shrink-0"
      onError={(event) => { event.currentTarget.style.visibility = 'hidden'; }} />
  );
}

/**
 * Lista, aplica, cria, edita e apaga páginas de runas (#26). Só mostra o
 * estado confirmado pelo agente; o rascunho do editor é a única coisa
 * local, e some quando a conexão com o LoL muda.
 */
export function RunePagesPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <section aria-label="Páginas de runas" className="w-full max-w-2xl rounded border border-slate-700 p-4">
        <h2 className="text-xl font-semibold">Páginas de runas</h2>
        <p>Aguarde a reconexão. As páginas serão confirmadas novamente pelo agente.</p>
      </section>
    );
  }
  if (!state.runePages) {
    return (
      <section aria-label="Páginas de runas" className="w-full max-w-2xl rounded border border-slate-700 p-4">
        <h2 className="text-xl font-semibold">Páginas de runas</h2>
        <p>{state.summoner ? 'Carregando páginas de runas…' : 'Aguarde o login no League of Legends.'}</p>
      </section>
    );
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
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  const busy = feedback.status === 'pending';
  const { pages, currentPageId, canCreatePage } = runePages;
  const editedPage = editing?.mode === 'edit' ? pages.find((page) => page.id === editing.pageId) : undefined;
  // A página em edição pode sumir por fora (apagada no LoL); aí o editor fecha.
  const openEditor = editing && catalog && (editing.mode === 'create' || editedPage?.isEditable) ? editing : null;

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
    <section aria-label="Páginas de runas" className="flex w-full max-w-2xl flex-col gap-3 rounded border border-slate-700 p-4">
      <h2 className="text-xl font-semibold">Páginas de runas</h2>
      {pages.length === 0
        ? <p>Nenhuma página de runas.</p>
        : (
          <ul aria-label="Páginas" className="flex flex-col gap-2">
            {pages.map((page) => (
              <PageItem key={page.id} page={page} catalog={catalog} current={page.id === currentPageId} busy={busy}
                editing={openEditor?.mode === 'edit' && openEditor.pageId === page.id}
                confirming={confirmDelete === page.id}
                onSelect={() => { void execute({ type: 'selectRunePage', pageId: page.id }); }}
                onEdit={() => { setConfirmDelete(null); setEditing({ mode: 'edit', pageId: page.id }); }}
                onDelete={() => setConfirmDelete(page.id)}
                onConfirmDelete={() => { void remove(page.id); }}
                onCancelDelete={() => setConfirmDelete(null)} />
            ))}
          </ul>
        )}
      {!openEditor && (
        <div className="flex flex-col gap-1">
          <button type="button" className={`${buttonClass} self-start`} disabled={!!createHint || busy}
            onClick={() => { setConfirmDelete(null); setEditing({ mode: 'create' }); }}>Nova página</button>
          {createHint && <p className="text-sm text-slate-300">{createHint}</p>}
        </div>
      )}
      {openEditor && catalog && (
        <RunePageEditor key={openEditor.mode === 'edit' ? openEditor.pageId : 'nova'} catalog={catalog} busy={busy}
          initial={editedPage ? draftFromPage(catalog, editedPage) : newDraft(catalog, `Página ${pages.length + 1}`)}
          title={editedPage ? `Editar ${editedPage.name}` : 'Nova página'}
          saveLabel={editedPage ? 'Salvar página' : 'Criar e aplicar'}
          onSave={(draft) => { void save(draft); }} onCancel={() => setEditing(null)} />
      )}
      {feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}>{feedback.message}</p>}
    </section>
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
  return (
    <li aria-label={page.name} aria-current={current ? 'true' : undefined}
      className={`flex flex-wrap items-center gap-2 rounded border p-2 ${current ? 'border-sky-400' : 'border-slate-700'}`}>
      <Icon kind="perk" id={page.selectedPerkIds[0] ?? 0} size={32} />
      <Icon kind="perkStyle" id={page.subStyleId} size={20} />
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="break-words font-medium">{page.name}</span>
        <span className="text-sm text-slate-300">
          {[summary, current && 'Em uso', page.isTemporary && 'Temporária', !page.isValid && 'Incompleta'].filter(Boolean).join(' · ')}
        </span>
      </div>
      {confirming
        ? (
          <>
            <span className="text-sm text-amber-300">Apagar esta página?</span>
            <button type="button" className={buttonClass} disabled={busy} onClick={onConfirmDelete}>Confirmar exclusão</button>
            <button type="button" className={buttonClass} onClick={onCancelDelete}>Manter</button>
          </>
        )
        : (
          <>
            <button type="button" className={buttonClass} aria-label={`Usar ${page.name}`} disabled={current || busy} onClick={onSelect}>Usar</button>
            {page.isEditable && (
              <button type="button" className={buttonClass} aria-label={`Editar ${page.name}`} disabled={!catalog || editing || busy} onClick={onEdit}>Editar</button>
            )}
            {page.isDeletable && (
              <button type="button" className={buttonClass} aria-label={`Apagar ${page.name}`} disabled={busy} onClick={onDelete}>Apagar</button>
            )}
          </>
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
  const primary = findStyle(catalog, draft.primaryStyleId);
  const sub = findStyle(catalog, draft.subStyleId);
  const problem = draftProblem(draft);
  const perk = (id: number) => catalog.perks.find((p) => p.id === id);

  function row(slot: RuneSlot, legend: string, selected: (perkId: number) => boolean, choose: (perkId: number) => void, key: string) {
    return (
      <fieldset key={key} className="flex flex-col gap-1">
        <legend className="text-sm text-slate-300">{legend}</legend>
        <div className="flex flex-wrap gap-1">
          {slot.perks.map((perkId) => {
            const info = perk(perkId);
            return (
              <button key={perkId} type="button" className={choiceClass} aria-pressed={selected(perkId)}
                title={info?.shortDesc || undefined} disabled={busy} onClick={() => choose(perkId)}>
                <Icon kind="perk" id={perkId} />
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
      <fieldset className="flex flex-col gap-1">
        <legend className="font-medium">{legend}</legend>
        <div className="flex flex-wrap gap-1">
          {options.map((style) => (
            <button key={style.id} type="button" className={choiceClass} aria-pressed={style.id === selected}
              disabled={busy} onClick={() => choose(style.id)}>
              <Icon kind="perkStyle" id={style.id} />
              {style.name}
            </button>
          ))}
        </div>
      </fieldset>
    );
  }

  return (
    <form aria-label={title} className="flex flex-col gap-3 rounded border border-slate-700 p-3"
      onSubmit={(event) => { event.preventDefault(); if (!problem && !busy) onSave(draft); }}>
      <h3 className="text-lg font-semibold">{title}</h3>
      <label className="flex flex-col gap-1">
        <span className="text-sm">Nome da página</span>
        <input className="rounded border border-slate-700 bg-slate-900 px-2 py-1" value={draft.name} maxLength={64}
          disabled={busy} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
      </label>
      {styles('Árvore primária', catalog.styles, draft.primaryStyleId,
        (id) => setDraft(withPrimary(catalog, draft, id)))}
      {primarySlots(primary).map((slot, index) => row(slot,
        slot.label || (slot.kind === 'keystone' ? 'Runa principal' : `Linha ${index}`),
        (perkId) => draft.primary[index] === perkId,
        (perkId) => setDraft(withPrimaryPerk(draft, index, perkId)), `p${index}`))}
      {styles('Árvore secundária', allowedSubStyles(catalog, primary), draft.subStyleId,
        (id) => setDraft(withSubStyle(draft, id)))}
      {subSlots(sub).map((slot, index) => row(slot,
        slot.label || `Secundária, linha ${index + 1}`,
        (perkId) => draft.sub.some((pick) => pick.row === index && pick.perkId === perkId),
        (perkId) => setDraft(withSubPerk(draft, index, perkId)), `s${index}`))}
      {statSlots(primary).map((slot, index) => row(slot,
        slot.label || `Fragmento ${index + 1}`,
        (perkId) => draft.stats[index] === perkId,
        (perkId) => setDraft(withStatPerk(draft, index, perkId)), `f${index}`))}
      {problem && <p className="text-sm text-amber-300">{problem}</p>}
      <div className="flex gap-2">
        <button type="submit" className={buttonClass} aria-busy={busy} disabled={!!problem || busy}>{saveLabel}</button>
        <button type="button" className={buttonClass} onClick={onCancel}>Cancelar</button>
      </div>
    </form>
  );
}

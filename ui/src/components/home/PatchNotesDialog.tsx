import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import type { AgentClient } from '../../agent/client';
import type { PatchChange } from '../../bindings/PatchChange';
import type { PatchNotes } from '../../bindings/PatchNotes';
import { Button } from '../base';

/**
 * Card com as notas da atualização (#385), aberto pelo "Ler as notas" do
 * Início. Mostra as seções que o agente leu da página oficial; "Abrir no
 * site" pede ao agente para abrir a página no navegador, porque a janela do
 * app não abre links.
 */
export function PatchNotesDialog({ notes, send, onClose }: {
  notes: PatchNotes;
  send: AgentClient['send'];
  onClose: () => void;
}) {
  const titleId = useId();
  const closeRef = useRef<HTMLButtonElement>(null);
  const mounted = useRef(true);
  const [error, setError] = useState(false);
  const [opening, setOpening] = useState(false);

  useEffect(() => {
    mounted.current = true;
    closeRef.current?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') onClose(); }
    document.addEventListener('keydown', onKey);
    return () => {
      mounted.current = false;
      document.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  async function openSite() {
    setOpening(true);
    setError(false);
    try {
      const result = await send({ type: 'openPatchNotes' });
      if (mounted.current) setError(result.status !== 'ok');
    } catch {
      if (mounted.current) setError(true);
    } finally {
      if (mounted.current) setOpening(false);
    }
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4 animate-fade-up"
      onPointerDown={event => { if (event.target === event.currentTarget) onClose(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby={titleId}
        className="flex max-h-[min(44rem,calc(100vh-2rem))] w-full max-w-2xl flex-col rounded-panel surface text-ink shadow-lift">
        <header className="flex items-start justify-between gap-3 border-b border-line p-5">
          <div className="flex min-w-0 flex-col gap-1">
            <p className="text-caption font-semibold uppercase tracking-wider text-muted">Notas da atualização {notes.version}</p>
            <h2 id={titleId} className="text-section font-semibold">{notes.title ?? `Atualização ${notes.version}`}</h2>
            {notes.summary && <p className="break-words text-body text-muted">{notes.summary}</p>}
          </div>
          <button ref={closeRef} type="button" aria-label="Fechar" onClick={onClose}
            className="flex size-8 shrink-0 items-center justify-center rounded-full text-muted hover:bg-raised hover:text-ink">
            <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
              strokeLinecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto px-5 pb-5">
          {notes.sections.length === 0
            ? <p className="pt-5 text-body text-muted">Não foi possível ler as mudanças desta atualização. Veja as notas completas no site oficial.</p>
            : notes.sections.map((section, i) => (
              <section key={i} aria-label={section.title} className="flex flex-col gap-3 pt-5">
                <h3 className="sticky top-0 z-10 -mx-5 bg-surface px-5 py-2 text-caption font-semibold uppercase tracking-wider text-accent-ink">
                  {section.title}
                </h3>
                {section.changes.map((change, j) => <Change key={j} change={change} />)}
              </section>
            ))}
        </div>
        <footer className="flex flex-wrap items-center justify-end gap-3 border-t border-line p-4">
          {error && <p role="alert" className="mr-auto text-caption text-danger">Não foi possível abrir o navegador.</p>}
          <Button size="sm" onClick={() => { void openSite(); }} loading={opening}>Abrir no site</Button>
          <Button size="sm" variant="primary" onClick={onClose}>Fechar</Button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}

function Change({ change }: { change: PatchChange }) {
  const bullets: string[] = [];
  const blocks: ReactNode[] = [];
  function flush(key: number) {
    if (bullets.length === 0) return;
    blocks.push(
      <ul key={`ul-${key}`} className="flex list-disc flex-col gap-1 pl-5 marker:text-muted">
        {bullets.splice(0).map((text, i) => <li key={i}>{text}</li>)}
      </ul>,
    );
  }
  change.lines.forEach((line, i) => {
    if (line.kind === 'bullet') {
      bullets.push(line.text);
      return;
    }
    flush(i);
    blocks.push(line.kind === 'heading'
      ? <h5 key={i} className="pt-1 text-body font-semibold">{line.text}</h5>
      : <p key={i} className="text-muted">{line.text}</p>);
  });
  flush(change.lines.length);
  return (
    <article className="flex flex-col gap-1.5 rounded-control bg-raised p-4 text-body break-words">
      {change.title && <h4 className="text-body font-semibold text-ink">{change.title}</h4>}
      {blocks}
    </article>
  );
}

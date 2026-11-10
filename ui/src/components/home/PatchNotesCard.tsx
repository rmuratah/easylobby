import { useCallback, useState } from 'react';
import type { AgentClient } from '../../agent/client';
import type { PatchNotes } from '../../bindings/PatchNotes';
import { Button } from '../base';
import { PatchNotesDialog } from './PatchNotesDialog';

/**
 * Notas da atualização atual (#301), no primeiro card pequeno do Início. O
 * agente lê a versão na LCU e as notas no site oficial; "Ler as notas" abre
 * um card com as mudanças dentro do app (#385).
 */
export function PatchNotesCard({ notes, send }: { notes: PatchNotes | null; send: AgentClient['send'] }) {
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  return (
    <article aria-label="Notas da atualização"
      className="flex min-h-32 min-w-0 flex-col gap-2 rounded-panel surface card p-5 shadow-soft animate-fade-up" style={{ animationDelay: '140ms' }}>
      <p className="text-caption font-semibold uppercase tracking-wider text-muted">Notas da atualização</p>
      {notes ? <>
        <h2 aria-label={`Atualização ${notes.version}`} className="text-display font-semibold leading-none text-ink">{notes.version}</h2>
        {notes.summary
          ? <p title={notes.summary} className="line-clamp-6 break-words text-body text-muted">{notes.summary}</p>
          : <p className="text-body text-muted">Veja o que mudou nesta atualização.</p>}
        <div className="mt-auto flex flex-col gap-1 pt-1">
          <Button size="sm" className="w-full" onClick={() => setOpen(true)}>Ler as notas</Button>
        </div>
        {open && <PatchNotesDialog notes={notes} send={send} onClose={close} />}
      </> : <p className="mt-auto text-body text-muted">As notas aparecem aqui assim que o LoL informar a versão do jogo.</p>}
    </article>
  );
}

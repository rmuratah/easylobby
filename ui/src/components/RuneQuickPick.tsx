import { useId } from 'react';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { RunePages } from '../bindings/RunePages';
import { Button, Select } from './base';
import { RuneIcon } from './RuneEditor';
import { findStyle } from './runePage';

type Props = {
  runePages: RunePages | null;
  catalog: RuneCatalog | null;
  send: AgentClient['send'];
  /** Abre a página de runas completa, sem sair da seleção (#131). */
  onEdit?: () => void;
};

/** Troca rápida da página de runas dentro da seleção de campeões (#131). */
export function RuneQuickPick({ runePages, catalog, send, onEdit }: Props) {
  const command = useCommand(send);
  const selectId = useId();
  const current = runePages?.pages.find(page => page.id === runePages.currentPageId);
  const keystone = current && catalog?.perks.find(perk => perk.id === current.selectedPerkIds[0]);
  const sub = current && catalog && findStyle(catalog, current.subStyleId);
  function choose(pageId: number) {
    if (command.pending || pageId === runePages?.currentPageId) return;
    void command.run({ type: 'selectRunePage', pageId }, { pending: 'Aplicando página…', failure: 'aplicar a página' });
  }
  return (
    <section aria-label="Suas runas" className="flex flex-col gap-2">
      <h3 className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">Runas</h3>
      {!runePages
        ? <p className="text-muted">Carregando páginas de runas…</p>
        : (
          <>
            <div className="flex items-center gap-2">
              <span className="relative shrink-0">
                <RuneIcon kind="perk" id={current?.selectedPerkIds[0] ?? 0} size={32} />
                <RuneIcon kind="perkStyle" id={current?.subStyleId ?? 0} size={16} className="absolute -bottom-1 -right-1 border-2 border-surface" />
              </span>
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <label htmlFor={selectId} className="sr-only">Página de runas</label>
                <Select id={selectId} size="sm" className="w-full" value={runePages.currentPageId}
                  disabled={command.pending || runePages.pages.length === 0} placeholder="Escolha uma página"
                  options={runePages.pages.map(page => ({ value: page.id, label: page.name }))}
                  onChange={choose} />
                {(keystone || sub) && <span className="truncate text-caption text-muted">{[keystone?.name, sub?.name].filter(Boolean).join(' · ')}</span>}
              </div>
            </div>
            {onEdit && <Button size="sm" variant="ghost" className="self-start" onClick={onEdit}>Editar runas</Button>}
          </>
        )}
      {command.feedback.status !== 'idle' && (
        <p role={command.feedback.status === 'error' ? 'alert' : 'status'}
          className={command.feedback.status === 'error' ? 'text-danger' : 'text-muted'}>{command.feedback.message}</p>
      )}
    </section>
  );
}

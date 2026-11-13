import { useEffect, useId, useRef } from 'react';
import { createPortal } from 'react-dom';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { Card } from './base';
import { RunePageEditor } from './RuneEditor';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

const heading = <h2 className="text-title font-semibold tracking-[-0.015em]">Páginas de runas</h2>;

/**
 * Páginas de runas da seleção de campeões (#26), em todos os modos com
 * runas. Usa o mesmo editor do pop-up do Jogo Dinâmico (#420): escolher uma
 * página a aplica, e o cabeçalho renomeia, cria e exclui páginas. Só mostra
 * o estado confirmado pelo agente; o rascunho do editor é a única coisa
 * local, e some quando a conexão com o LoL muda.
 */
export function RunePagesPanel({ connection, send, bare = false }: Props & {
  /** Sem cartão nem título, dentro do pop-up (#429). */
  bare?: boolean;
}) {
  const state = connection.state;
  const lcu = state?.lcu;
  let body;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    body = <p className="text-body text-muted">Aguarde a reconexão. As páginas serão confirmadas novamente pelo agente.</p>;
  } else if (!state.runePages) {
    body = <p className="text-body text-muted">{state.summoner ? 'Carregando páginas de runas…' : 'Aguarde o login no League of Legends.'}</p>;
  } else if (!state.runeCatalog) {
    body = <p className="text-body text-muted">Carregando as árvores de runas…</p>;
  } else {
    body = (
      <RunePageEditor key={`${lcu.session}:${state.summoner?.puuid ?? ''}`}
        catalog={state.runeCatalog} runePages={state.runePages} send={send} />
    );
  }
  if (bare) return <section aria-label="Páginas de runas" className="flex flex-1 flex-col text-ink">{body}</section>;
  return (
    <Card as="section" aria-label="Páginas de runas" className="flex w-full max-w-[56rem] flex-col gap-4 p-5 text-ink">
      {heading}
      {body}
    </Card>
  );
}

/**
 * O editor de runas num pop-up por cima da seleção de campeões (#429): a
 * seleção continua na tela, com o timer correndo. Fecha pelo X, por Esc ou
 * clicando fora, e devolve o foco a quem abriu.
 */
export function RunesDialog({ connection, send, onClose }: Props & { onClose: () => void }) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  useEffect(() => { closeRef.current = onClose; });
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    // Foco na escolha de página; sem ela (carregando), no Fechar.
    (ref.current?.querySelector<HTMLElement>('select') ?? ref.current?.querySelector<HTMLElement>('button'))?.focus();
    // Esc tratado por um campo do editor (como desfazer o nome) não chega aqui.
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') closeRef.current(); }
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      opener?.focus?.();
    };
  }, []);
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4 animate-fade-up"
      onPointerDown={event => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby={titleId}
        className="flex h-[min(40rem,calc(100vh-2rem))] w-full max-w-5xl flex-col overflow-hidden rounded-panel bg-surface card text-ink shadow-lift">
        <header className="flex items-center justify-between gap-3 border-b border-line px-5 py-3">
          <h2 id={titleId} className="text-section font-semibold">Editar runas</h2>
          <button type="button" aria-label="Fechar" title="Fechar" onClick={onClose}
            className="flex size-8 shrink-0 items-center justify-center rounded-full text-muted hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent">
            <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
              strokeLinecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>
        {/* A janela usa quase toda a altura da tela e o editor estica junto, com mais espaço entre as runas (#475). */}
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4">
          <RunePagesPanel connection={connection} send={send} bare />
        </div>
      </div>
    </div>,
    document.body,
  );
}

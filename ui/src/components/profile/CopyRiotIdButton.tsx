import { useEffect, useState } from 'react';
import { cx } from '../base/cx';

type Props = { gameName: string; tagLine: string };

/**
 * Ícone ao lado do nome no perfil (#388): copia "nome#tag" para a área de
 * transferência, como no client oficial, e mostra um balão confirmando.
 * Se a cópia falhar, o balão mostra o Riot ID selecionável para copiar à mão.
 */
export function CopyRiotIdButton({ gameName, tagLine }: Props) {
  const riotId = `${gameName}#${tagLine}`;
  const [status, setStatus] = useState<'idle' | 'copied' | 'manual'>('idle');
  useEffect(() => {
    if (status !== 'copied') return;
    const timer = setTimeout(() => setStatus('idle'), 2500);
    return () => clearTimeout(timer);
  }, [status]);

  async function copy() {
    try {
      await navigator.clipboard.writeText(riotId);
      setStatus('copied');
    } catch {
      setStatus('manual');
    }
  }

  return (
    <span className="relative ml-1.5 inline-flex shrink-0">
      <button type="button" aria-label={`Copiar ${riotId}`} title="Copiar nome e tag"
        onClick={() => { void copy(); }} onBlur={() => setStatus(s => (s === 'manual' ? s : 'idle'))}
        className={cx('inline-flex size-6 items-center justify-center rounded-tile transition-colors hover:bg-raised hover:text-ink',
          'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent',
          status === 'copied' ? 'text-accent-ink' : 'text-muted')}>
        <svg aria-hidden="true" viewBox="0 0 16 16" className="size-4" fill="currentColor">
          <path d="M5 1.5A1.5 1.5 0 0 1 6.5 0h6A1.5 1.5 0 0 1 14 1.5v9a1.5 1.5 0 0 1-1.5 1.5h-6A1.5 1.5 0 0 1 5 10.5v-9Z" />
          <path d="M3.5 3.5H4v7A2.5 2.5 0 0 0 6.5 13H11v.5A1.5 1.5 0 0 1 9.5 15h-6A1.5 1.5 0 0 1 2 13.5v-8.5a1.5 1.5 0 0 1 1.5-1.5Z" opacity=".55" />
        </svg>
      </button>
      {status !== 'idle' && (
        <span role="status"
          className="absolute left-1/2 top-full z-20 mt-2 w-max max-w-72 -translate-x-1/2 rounded-tile border border-edge bg-surface px-4 py-2 text-center text-caption font-normal shadow-lift">
          <span className="block select-all text-muted">{gameName} #{tagLine}</span>
          <span className="mt-1.5 block border-t border-line pt-1.5 text-ink">
            {status === 'copied' ? 'Copiado para a área de transferência' : 'Não deu para copiar. Selecione e copie à mão.'}
          </span>
        </span>
      )}
    </span>
  );
}

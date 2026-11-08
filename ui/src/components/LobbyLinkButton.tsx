import { useEffect, useRef, useState } from 'react';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { LobbyLink } from '../bindings/LobbyLink';
import { cx } from './base/cx';

type Props = { link: LobbyLink | null; send: AgentClient['send']; disabled?: boolean };

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';

/**
 * Botão "Copiar link" do lobby (#226): pede o link ao agente e copia para a
 * área de transferência, para outra pessoa entrar no lobby. O link chega no
 * estado antes da resposta do comando; se a cópia falhar, ele aparece na tela
 * para copiar à mão.
 */
export function LobbyLinkButton({ link, send, disabled = false }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const latest = useRef(link);
  useEffect(() => { latest.current = link; }, [link]);
  const [copied, setCopied] = useState<'idle' | 'copied' | 'manual'>('idle');
  useEffect(() => {
    if (copied !== 'copied') return;
    const timer = setTimeout(() => setCopied('idle'), 2500);
    return () => clearTimeout(timer);
  }, [copied]);

  async function copy() {
    setCopied('idle');
    const ok = await run({ type: 'createLobbyLink' }, { pending: 'Gerando o link…', failure: 'gerar o link' });
    const url = latest.current?.url;
    if (!ok || !url) return;
    try {
      await navigator.clipboard.writeText(url);
      setCopied('copied');
    } catch {
      setCopied('manual');
    }
  }

  return (
    <span className="inline-flex flex-col items-end gap-1">
      <button type="button" aria-busy={pending} disabled={disabled || pending}
        title="Copia um link para outra pessoa entrar neste lobby."
        onClick={() => { void copy(); }}
        className={cx('inline-flex h-10 items-center rounded-full surface px-3 text-caption font-semibold shadow-soft transition-colors disabled:cursor-not-allowed', focusRing,
          copied === 'copied' ? 'text-accent-ink' : 'text-muted')}>
        {copied === 'copied' ? 'Link copiado' : 'Copiar link'}
      </button>
      {copied === 'manual' && link && (
        <span className="max-w-64 select-all break-all text-right text-caption text-muted">{link.url}</span>
      )}
      {feedback.status === 'error' && <span role="alert" className="max-w-64 text-right text-caption text-danger">{feedback.message}</span>}
    </span>
  );
}

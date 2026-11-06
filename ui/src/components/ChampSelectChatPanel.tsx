import { useEffect, useId, useLayoutEffect, useRef, useState, type FormEvent } from 'react';
import { messageTime } from '../agent/chat';
import { playerName } from '../agent/champSelect';
import type { AgentClient } from '../agent/client';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectChat } from '../bindings/ChampSelectChat';
import type { ChampSelectChatMessage } from '../bindings/ChampSelectChatMessage';

type Props = { champSelect: ChampSelect; chat: ChampSelectChat | null; send: AgentClient['send'] };

function senderName(champSelect: ChampSelect, message: ChampSelectChatMessage): string {
  if (message.fromMe) return 'Você';
  const index = champSelect.myTeam.findIndex(p => p.cellId === message.fromCellId);
  return index < 0 ? 'Aliado' : playerName(champSelect.myTeam[index], index, true);
}

/** Chat do time na seleção de campeões (#27). */
export function ChampSelectChatPanel({ champSelect, chat, send }: Props) {
  const [draft, setDraft] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);
  const log = useRef<HTMLOListElement>(null);
  const inputId = useId();
  const messages = chat?.messages ?? [];
  const lastId = messages.at(-1)?.id;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  // Desce até a mensagem mais nova quando chega mensagem.
  useLayoutEffect(() => {
    const element = log.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [lastId]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const body = draft.trim();
    if (!body || sending || !chat) return;
    setSending(true);
    setError(null);
    let failure: string | null = null;
    try {
      const result = await send({ type: 'sendChampSelectMessage', body });
      if (result.status === 'error') failure = result.message;
    } catch (e) {
      failure = e instanceof Error ? e.message : String(e);
    }
    if (!mounted.current) return;
    setSending(false);
    if (failure === null) setDraft(current => current.trim() === body ? '' : current);
    else setError(`Não foi possível enviar a mensagem: ${failure}`);
  }

  return (
    <section aria-label="Chat do time" className="flex min-w-0 flex-col gap-2">
      <h3 className="text-lg font-semibold">Chat do time</h3>
      {!chat ? <p className="text-slate-300">Aguardando a sala do chat abrir…</p>
        : messages.length === 0 ? <p className="text-slate-300">Nenhuma mensagem ainda.</p>
          : (
            <ol ref={log} role="log" aria-label="Mensagens do time" className="flex max-h-64 flex-col gap-1 overflow-y-auto pr-1">
              {messages.map(message => (
                <li key={message.id} className="break-words text-sm">
                  <span className="text-xs text-slate-400">{messageTime(message)} </span>
                  <span className={`font-semibold ${message.fromMe ? 'text-sky-300' : 'text-slate-200'}`}>{senderName(champSelect, message)}: </span>
                  <span className="whitespace-pre-wrap">{message.body}</span>
                </li>
              ))}
            </ol>
          )}
      <form onSubmit={submit} className="flex gap-2">
        <label htmlFor={inputId} className="sr-only">Mensagem para o time</label>
        <input id={inputId} type="text" value={draft} autoComplete="off" placeholder="Mensagem para o time"
          disabled={!chat} onChange={event => setDraft(event.target.value)}
          className="min-w-0 flex-1 rounded border border-slate-600 bg-slate-900 px-2 py-1 disabled:opacity-50" />
        <button type="submit" disabled={!chat || !draft.trim() || sending}
          className="rounded border border-sky-400 px-3 py-1 disabled:cursor-not-allowed disabled:opacity-50">
          {sending ? 'Enviando…' : 'Enviar'}
        </button>
      </form>
      {error && <p role="alert" className="text-amber-300">{error}</p>}
    </section>
  );
}

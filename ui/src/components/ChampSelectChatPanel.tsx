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
export function ChampSelectChatPanel(props: Props) {
  if (!props.chat) return (
    <section aria-label="Chat do time" className="flex min-w-0 flex-col gap-2">
      <h3 className="text-lg font-semibold">Chat do time</h3>
      <p className="text-slate-300">Aguardando a sala do chat abrir…</p>
      <input aria-label="Mensagem para o time" disabled />
      <button disabled>Enviar</button>
    </section>
  );
  // Fechar a sala desmonta o formulário e invalida respostas ainda em trânsito.
  return <OpenChat key={props.champSelect.id} {...props} chat={props.chat} />;
}

function OpenChat({ champSelect, chat, send }: Props & { chat: ChampSelectChat }) {
  const [draft, setDraft] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);
  const pending = useRef(false);
  const follow = useRef(true);
  const log = useRef<HTMLOListElement>(null);
  const inputId = useId();
  const messages = chat.messages;
  const lastId = messages.at(-1)?.id;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  // Só acompanha o fim enquanto o usuário não estiver lendo o histórico.
  useLayoutEffect(() => {
    const element = log.current;
    if (element && follow.current) element.scrollTop = element.scrollHeight;
  }, [lastId]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const body = draft.trim();
    if (!body || pending.current) return;
    pending.current = true;
    setSending(true);
    setError(null);
    let failure: string | null = null;
    try {
      const result = await send({ type: 'sendChampSelectMessage', body });
      if (result.status === 'error') failure = `Não foi possível enviar a mensagem: ${result.message}`;
    } catch (e) {
      failure = `Não foi possível confirmar: ${e instanceof Error ? e.message : String(e)} Confira o histórico antes de tentar novamente.`;
    }
    if (!mounted.current) return;
    pending.current = false;
    setSending(false);
    if (failure === null) setDraft(current => current.trim() === body ? '' : current);
    else setError(failure);
  }

  return (
    <section aria-label="Chat do time" className="flex min-w-0 flex-col gap-2">
      <h3 className="text-lg font-semibold">Chat do time</h3>
      {messages.length === 0 ? <p className="text-slate-300">Nenhuma mensagem ainda.</p>
          : (
            <ol ref={log} onScroll={() => {
              const element = log.current;
              if (element) follow.current = element.scrollHeight - element.scrollTop - element.clientHeight <= 32;
            }} role="log" aria-label="Mensagens do time" className="flex max-h-64 flex-col gap-1 overflow-y-auto pr-1">
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
          onChange={event => setDraft(event.target.value)}
          className="min-w-0 flex-1 rounded border border-slate-600 bg-slate-900 px-2 py-1 disabled:opacity-50" />
        <button type="submit" disabled={!draft.trim() || sending}
          className="rounded border border-sky-400 px-3 py-1 disabled:cursor-not-allowed disabled:opacity-50">
          {sending ? 'Enviando…' : 'Enviar'}
        </button>
      </form>
      {error && <p role="alert" className="text-amber-300">{error}</p>}
    </section>
  );
}

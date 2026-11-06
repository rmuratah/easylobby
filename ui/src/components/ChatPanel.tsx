import { useEffect, useId, useLayoutEffect, useRef, useState, type FormEvent } from 'react';
import { conversationName, messageTime, sortConversations, totalUnread } from '../agent/chat';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { friendDisplayName } from '../agent/friends';
import type { Conversation } from '../bindings/Conversation';
import type { Friend } from '../bindings/Friend';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /**
   * Conversa aberta (o `id` da conversa, que é o `pid` do amigo), ou `null`
   * para mostrar a lista. Fica fora do painel porque a lista de amigos também
   * abre conversas.
   */
  activeId: string | null;
  onActiveChange: (id: string | null) => void;
};

const buttonClass = 'rounded border border-sky-400 px-3 py-1 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-4';
const noop = () => {};

/**
 * Chat 1:1 com amigos (#31): as conversas abertas, com as não lidas, e a
 * conversa escolhida com o histórico da sessão e o campo para responder.
 */
export function ChatPanel({ connection, send, activeId, onActiveChange }: Props) {
  const state = connection.state;
  const connected = connection.status === 'connected' && state?.lcu.state === 'connected';
  const session = state?.lcu.state === 'connected' ? state.lcu.session : null;
  const conversations = connected ? state.conversations : [];
  const friends = connected ? state.friends : [];

  // Conversa com um amigo que ainda não tem conversa aberta na LCU: a
  // primeira mensagem abre.
  const existing = activeId !== null ? conversations.find(c => c.id === activeId) : undefined;
  const friend = activeId !== null && !existing ? friends.find(f => f.id === activeId) : undefined;
  const active: Conversation | null = existing ?? (friend ? newConversation(friend) : null);
  const exists = !!existing;

  // Avisa a LCU qual conversa o jogador está vendo, para ela zerar as não
  // lidas, e desmarca ao sair dela. Só vale para conversas que já existem.
  useEffect(() => {
    if (!connected || activeId === null || !exists) return;
    send({ type: 'setActiveConversation', conversationId: activeId }).catch(noop);
    return () => { send({ type: 'setActiveConversation', conversationId: null }).catch(noop); };
  }, [send, connected, session, activeId, exists]);

  if (!connected) {
    return (
      <section aria-label="Conversas" className="w-full max-w-lg rounded border border-slate-700 p-4">
        <h2 className="text-xl font-semibold">Conversas</h2>
        <p className="text-slate-300">Aguarde a conexão com o agente e o League of Legends para conversar com os amigos.</p>
      </section>
    );
  }

  if (active) {
    return (
      <ConversationView
        key={active.id} conversation={active} name={conversationName(active, friends)} send={send}
        onBack={() => onActiveChange(null)}
      />
    );
  }

  const unread = totalUnread(conversations);
  return (
    <section aria-label="Conversas" className="flex w-full max-w-lg flex-col gap-3 rounded border border-slate-700 p-4">
      <h2 className="text-xl font-semibold">
        Conversas{unread > 0 && <span className="text-base font-normal text-slate-300"> ({unread} não {unread === 1 ? 'lida' : 'lidas'})</span>}
      </h2>
      {conversations.length === 0
        ? <p className="text-slate-300">Nenhuma conversa aberta. Use “Conversar” na lista de amigos para começar uma.</p>
        : (
          <ul aria-label="Conversas abertas" className="flex flex-col gap-1">
            {sortConversations(conversations, friends).map(conversation => {
              const name = conversationName(conversation, friends);
              const last = conversation.messages.at(-1);
              return (
                <li key={conversation.id}>
                  <button
                    type="button" onClick={() => onActiveChange(conversation.id)}
                    aria-label={`Abrir conversa com ${name}${conversation.unreadCount > 0 ? `, ${conversation.unreadCount} não ${conversation.unreadCount === 1 ? 'lida' : 'lidas'}` : ''}`}
                    className="flex w-full items-center gap-3 rounded px-2 py-1 text-left hover:bg-slate-900 focus-visible:outline-2 focus-visible:outline-offset-2"
                  >
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className={`truncate ${conversation.unreadCount > 0 ? 'font-semibold' : 'font-medium'}`} title={name}>{name}</span>
                      {last && (
                        <span className="truncate text-sm text-slate-300">{last.fromMe ? 'Você: ' : ''}{last.body}</span>
                      )}
                    </span>
                    {last && <span className="shrink-0 text-xs text-slate-400">{messageTime(last)}</span>}
                    {conversation.unreadCount > 0 && (
                      <span aria-hidden="true" className="shrink-0 rounded-full bg-sky-500 px-2 text-xs font-semibold text-slate-950">
                        {conversation.unreadCount}
                      </span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
    </section>
  );
}

function newConversation(friend: Friend): Conversation {
  return { id: friend.id, gameName: friend.gameName, tagLine: friend.tagLine, unreadCount: 0, messages: [] };
}

function ConversationView({ conversation, name, send, onBack }: {
  conversation: Conversation; name: string; send: AgentClient['send']; onBack: () => void;
}) {
  const [draft, setDraft] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);
  const log = useRef<HTMLOListElement>(null);
  const inputId = useId();
  const messages = conversation.messages;
  const lastId = messages.at(-1)?.id;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  // Desce até a mensagem mais nova ao abrir e quando chega mensagem.
  useLayoutEffect(() => {
    const element = log.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [lastId]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const body = draft.trim();
    if (!body || sending) return;
    setSending(true);
    setError(null);
    let message: string | null = null;
    try {
      const result = await send({ type: 'sendChatMessage', conversationId: conversation.id, body });
      if (result.status === 'error') message = result.message;
    } catch (e) {
      message = e instanceof Error ? e.message : String(e);
    }
    if (!mounted.current) return;
    setSending(false);
    if (message === null) setDraft(current => current.trim() === body ? '' : current);
    else setError(`Não foi possível enviar a mensagem: ${message}`);
  }

  return (
    <section aria-label={`Conversa com ${name}`} className="flex w-full max-w-lg flex-col gap-3 rounded border border-slate-700 p-4">
      <div className="flex items-center gap-3">
        <button type="button" onClick={onBack} className={buttonClass} aria-label="Voltar às conversas">←</button>
        <h2 className="min-w-0 truncate text-xl font-semibold" title={name}>{name}</h2>
      </div>
      {messages.length === 0
        ? <p className="text-slate-300">Nenhuma mensagem nesta sessão. Mande a primeira.</p>
        : (
          <ol ref={log} role="log" aria-label="Mensagens" className="flex max-h-80 flex-col gap-2 overflow-y-auto pr-1">
            {messages.map(message => (
              <li key={message.id} className={`flex flex-col ${message.fromMe ? 'items-end' : 'items-start'}`}>
                <span className="sr-only">{message.fromMe ? 'Você' : name}: </span>
                <span className={`max-w-[85%] rounded px-3 py-1 break-words whitespace-pre-wrap ${message.fromMe ? 'bg-sky-900' : 'bg-slate-800'}`}>
                  {message.body}
                </span>
                <span className="text-xs text-slate-400">{messageTime(message)}</span>
              </li>
            ))}
          </ol>
        )}
      <form onSubmit={submit} className="flex gap-2">
        <label htmlFor={inputId} className="sr-only">Mensagem para {name}</label>
        <input
          id={inputId} type="text" value={draft} autoComplete="off" placeholder="Escreva uma mensagem"
          onChange={event => setDraft(event.target.value)}
          className="min-w-0 flex-1 rounded border border-slate-600 bg-slate-900 p-2"
        />
        <button type="submit" disabled={!draft.trim() || sending} className={buttonClass}>
          {sending ? 'Enviando…' : 'Enviar'}
        </button>
      </form>
      {error && <p role="alert" className="text-amber-300">{error}</p>}
    </section>
  );
}

/** Botão "Conversar" de cada amigo da lista de amigos. */
export function ChatButton({ friend, onOpen }: { friend: Friend; onOpen: (id: string) => void }) {
  return (
    <button
      type="button" onClick={() => onOpen(friend.id)} aria-label={`Conversar com ${friendDisplayName(friend)}`}
      className="rounded border border-sky-400 px-2 py-0.5 text-sm focus-visible:outline-2 focus-visible:outline-offset-2"
    >
      Conversar
    </button>
  );
}

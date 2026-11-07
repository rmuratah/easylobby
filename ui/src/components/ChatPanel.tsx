import { useEffect, useId, useLayoutEffect, useRef, useState, type FormEvent } from 'react';
import { conversationName, messageTime, sortConversations, totalUnread } from '../agent/chat';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { friendNick } from '../agent/friends';
import type { Conversation } from '../bindings/Conversation';
import type { Friend } from '../bindings/Friend';
import { Avatar, Button, Card } from './base';
import { cx } from './base/cx';

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
      <Card as="section" aria-label="Conversas" className="flex w-full flex-col gap-2 p-4 text-ink">
        <h2 className="text-section font-semibold">Conversas</h2>
        <p className="text-body text-muted">Aguarde a conexão com o agente e o League of Legends para conversar com os amigos.</p>
      </Card>
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
    <Card as="section" aria-label="Conversas" className="flex w-full flex-col gap-3 p-4 text-ink">
      <h2 className="text-section font-semibold">
        Conversas{unread > 0 && <span className="text-caption font-normal text-muted"> ({unread} não {unread === 1 ? 'lida' : 'lidas'})</span>}
      </h2>
      {conversations.length === 0
        ? <p className="text-body text-muted">Nenhuma conversa aberta. Use “Conversar” na lista de amigos para começar uma.</p>
        : (
          <ul aria-label="Conversas abertas" className="-mx-2 flex flex-col">
            {sortConversations(conversations, friends).map(conversation => {
              const name = conversationName(conversation, friends);
              const last = conversation.messages.at(-1);
              return (
                <li key={conversation.id}>
                  <button
                    type="button" onClick={() => onActiveChange(conversation.id)}
                    aria-label={`Abrir conversa com ${name}${conversation.unreadCount > 0 ? `, ${conversation.unreadCount} não ${conversation.unreadCount === 1 ? 'lida' : 'lidas'}` : ''}`}
                    className="flex w-full items-center gap-3 rounded-control px-2 py-1.5 text-left text-ink hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
                  >
                    <Avatar name={name} size={32} />
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className={cx('truncate text-body', conversation.unreadCount > 0 ? 'font-semibold' : 'font-medium')} title={name}>{name}</span>
                      {last && (
                        <span className="truncate text-caption text-muted">{last.fromMe ? 'Você: ' : ''}{last.body}</span>
                      )}
                    </span>
                    {last && <span className="shrink-0 text-caption text-muted">{messageTime(last)}</span>}
                    {conversation.unreadCount > 0 && (
                      <span aria-hidden="true" className="shrink-0 rounded-full bg-accent px-2 text-caption font-semibold text-on-accent">
                        {conversation.unreadCount}
                      </span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
    </Card>
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
    <Card as="section" aria-label={`Conversa com ${name}`} className="flex w-full flex-col gap-3 p-4 text-ink">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onBack} aria-label="Voltar às conversas" className="px-2">←</Button>
        <Avatar name={name} size={28} />
        <h2 className="min-w-0 truncate text-section font-semibold" title={name}>{name}</h2>
      </div>
      {messages.length === 0
        ? <p className="text-body text-muted">Nenhuma mensagem nesta sessão. Mande a primeira.</p>
        : (
          <ol ref={log} role="log" aria-label="Mensagens" className="flex max-h-80 flex-col gap-2 overflow-y-auto pr-1">
            {messages.map(message => (
              <li key={message.id} className={cx('flex flex-col gap-0.5', message.fromMe ? 'items-end' : 'items-start')}>
                <span className="sr-only">{message.fromMe ? 'Você' : name}: </span>
                <span className={cx('max-w-[85%] rounded-card px-3 py-1.5 text-body break-words whitespace-pre-wrap',
                  message.fromMe ? 'rounded-br-control-sm bg-accent text-on-accent' : 'rounded-bl-control-sm bg-raised text-ink')}>
                  {message.body}
                </span>
                <span className="text-caption text-muted">{messageTime(message)}</span>
              </li>
            ))}
          </ol>
        )}
      <form onSubmit={submit} className="flex gap-2">
        <label htmlFor={inputId} className="sr-only">Mensagem para {name}</label>
        <input
          id={inputId} type="text" value={draft} autoComplete="off" placeholder="Escreva uma mensagem"
          onChange={event => setDraft(event.target.value)}
          className="h-9 min-w-0 flex-1 rounded-control border border-control bg-surface px-3 text-body text-ink placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
        />
        <Button type="submit" variant="primary" size="sm" className="h-9" disabled={!draft.trim() || sending}>
          {sending ? 'Enviando…' : 'Enviar'}
        </Button>
      </form>
      {error && <p role="alert" className="text-body text-danger">{error}</p>}
    </Card>
  );
}

/** Botão "Conversar" de cada amigo da lista de amigos: só o ícone, para a coluna estreita caber o status. */
export function ChatButton({ friend, onOpen }: { friend: Friend; onOpen: (id: string) => void }) {
  const label = `Conversar com ${friendNick(friend)}`;
  return (
    <Button size="sm" variant="ghost" onClick={() => onOpen(friend.id)} aria-label={label} title={label}>
      <svg aria-hidden="true" className="shrink-0" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z" />
      </svg>
    </Button>
  );
}

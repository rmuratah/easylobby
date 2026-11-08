import { useEffect, useId, useLayoutEffect, useRef, useState, type FormEvent } from 'react';
import { conversationName, messageTime, sortConversations } from '../agent/chat';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { friendNick } from '../agent/friends';
import type { Conversation } from '../bindings/Conversation';
import type { Friend } from '../bindings/Friend';
import { Avatar, Button, Card, EmptyState } from './base';
import { cx } from './base/cx';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /**
   * Conversa aberta (o `id` da conversa, que é o `pid` do amigo), ou `null`
   * para só mostrar a lista. Fica fora da tela porque o balão do amigo e os
   * avisos de mensagem também abrem conversas.
   */
  activeId: string | null;
  onActiveChange: (id: string | null) => void;
  /**
   * A tela está à vista. Escondida, ela continua montada (o rascunho não se
   * perde), mas não avisa a LCU que a conversa está sendo lida.
   */
  visible?: boolean;
};

const noop = () => {};

/**
 * Tela de Conversas (#251): a lista de conversas da sessão, da mais recente
 * para a mais antiga, e a conversa aberta ao lado. Em janela estreita, mostra
 * uma de cada vez, com a seta para voltar à lista.
 */
export function ConversationsPage({ connection, send, activeId, onActiveChange, visible = true }: Props) {
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
  // lidas, e desmarca ao sair dela ou da tela. Só vale para conversas que já existem.
  useEffect(() => {
    if (!connected || !visible || activeId === null || !exists) return;
    send({ type: 'setActiveConversation', conversationId: activeId }).catch(noop);
    return () => { send({ type: 'setActiveConversation', conversationId: null }).catch(noop); };
  }, [send, connected, session, visible, activeId, exists]);

  if (!connected) {
    return <EmptyState title="Conversas" description="Aguarde a conexão com o agente e o League of Legends para ver as conversas." />;
  }

  const sorted = sortConversations(conversations, friends);
  return (
    <div className="grid h-full min-h-0 gap-4 @min-[44rem]:grid-cols-[minmax(14rem,18rem)_minmax(0,1fr)]">
      <Card as="section" aria-label="Lista de conversas"
        className={cx('flex min-h-0 flex-col gap-2 overflow-y-auto p-3 text-ink', active && '@max-[44rem]:hidden')}>
        <h2 className="px-1 text-section font-semibold">Conversas</h2>
        {sorted.length === 0
          ? <p className="px-1 text-body text-muted">Nenhuma conversa nesta sessão. Use o balão de conversar na lista de amigos para começar.</p>
          : (
            <ul aria-label="Conversas abertas" className="flex flex-col gap-1">
              {sorted.map(conversation => (
                <li key={conversation.id}>
                  <ConversationItem conversation={conversation} name={conversationName(conversation, friends)}
                    selected={conversation.id === active?.id} onOpen={() => onActiveChange(conversation.id)} />
                </li>
              ))}
            </ul>
          )}
      </Card>
      {active
        ? <ConversationView key={active.id} conversation={active} name={conversationName(active, friends)} send={send}
            onBack={() => onActiveChange(null)} />
        : (
          <Card className="hidden min-h-0 items-center justify-center p-6 text-center @min-[44rem]:flex">
            <p className="text-body text-muted">Escolha uma conversa na lista.</p>
          </Card>
        )}
    </div>
  );
}

function ConversationItem({ conversation, name, selected, onOpen }: {
  conversation: Conversation; name: string; selected: boolean; onOpen: () => void;
}) {
  const last = conversation.messages.at(-1);
  const unread = conversation.unreadCount;
  const label = `Abrir conversa com ${name}${unread > 0 ? `, ${unread} não ${unread === 1 ? 'lida' : 'lidas'}` : ''}`;
  return (
    <button type="button" onClick={onOpen} aria-label={label} aria-current={selected ? 'true' : undefined}
      className={cx('flex w-full items-center gap-3 rounded-control px-2 py-2 text-left transition-colors',
        'focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent',
        selected ? 'bg-accent-soft text-accent-ink' : 'hover:bg-raised')}>
      <Avatar name={name} size={32} />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-body font-semibold">{name}</span>
        <span className="truncate text-caption text-muted">
          {last ? `${last.fromMe ? 'Você: ' : ''}${last.body}` : 'Nenhuma mensagem ainda'}
        </span>
      </span>
      {unread > 0 && <span aria-hidden="true" className="min-w-5 shrink-0 rounded-full bg-accent px-1.5 text-center text-caption font-semibold text-on-accent">{unread}</span>}
    </button>
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
    <Card as="section" aria-label={`Conversa com ${name}`} className="flex min-h-0 w-full flex-col gap-3 p-4 text-ink">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={onBack} aria-label="Voltar às conversas" title="Voltar às conversas" className="px-2 @min-[44rem]:hidden">←</Button>
        <Avatar name={name} size={28} />
        <h2 className="min-w-0 truncate text-section font-semibold" title={name}>{name}</h2>
      </div>
      {messages.length === 0
        ? <p className="flex-1 text-body text-muted">Nenhuma mensagem nesta sessão. Mande a primeira.</p>
        : (
          <ol ref={log} role="log" aria-label="Mensagens" className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto pr-1">
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

/**
 * Botão "Conversar" de cada amigo da lista de amigos: só o ícone, para a coluna
 * estreita caber o status. Com mensagens não lidas, mostra um ponto de destaque.
 */
export function ChatButton({ friend, unread = 0, onOpen }: { friend: Friend; unread?: number; onOpen: (id: string) => void }) {
  const label = `Conversar com ${friendNick(friend)}${unread > 0 ? `, ${unread} não ${unread === 1 ? 'lida' : 'lidas'}` : ''}`;
  return (
    <Button size="sm" variant="ghost" onClick={() => onOpen(friend.id)} aria-label={label} title={label} className="relative">
      <svg aria-hidden="true" className="shrink-0" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z" />
      </svg>
      {unread > 0 && <span aria-hidden="true" data-unread className="absolute right-1 top-1 h-2 w-2 rounded-full bg-accent" />}
    </Button>
  );
}

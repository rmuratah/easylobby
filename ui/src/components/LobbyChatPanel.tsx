import { useEffect, useId, useLayoutEffect, useRef, useState, type FormEvent } from 'react';
import { messageTime } from '../agent/chat';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { playerName } from '../agent/friends';
import type { AgentState } from '../bindings/AgentState';
import type { LobbyChat } from '../bindings/LobbyChat';
import type { LobbyChatMessage } from '../bindings/LobbyChatMessage';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

// Riot ID como no client oficial ("Nome #tag"). O agente preenche o nome; até lá, o do jogador logado ou da lista de amigos.
function senderName(state: AgentState, message: LobbyChatMessage): string {
  if (message.gameName) return message.tagLine ? `${message.gameName} #${message.tagLine}` : message.gameName;
  const me = state.summoner;
  if (message.fromMe && me?.gameName) return me.tagLine ? `${me.gameName} #${me.tagLine}` : me.gameName;
  const member = state.lobby?.members.find(m => m.puuid === message.fromPuuid);
  return playerName(message.fromPuuid, member?.summonerId ?? 0, state.friends);
}

function ChatLine({ state, message }: { state: AgentState; message: LobbyChatMessage }) {
  const name = senderName(state, message);
  const time = messageTime(message);
  if (message.kind !== 'chat') return (
    <li title={time} className="break-words text-body text-muted">
      {name} {message.kind === 'joined' ? 'entrou no saguão' : 'saiu do saguão'}
    </li>
  );
  return (
    <li title={time} className="break-words text-body">
      <span className={`font-semibold ${message.fromMe ? 'text-accent-ink' : 'text-ink'}`}>{name}</span>
      <span className="text-muted">: </span>
      <span className="whitespace-pre-wrap">{message.body}</span>
    </li>
  );
}

const cardClass = 'flex w-full min-w-0 flex-col gap-2 rounded-panel surface p-4 text-ink shadow-soft animate-fade-up';

/**
 * Chat do grupo no lobby (#206), embaixo da busca de partida e do card do grupo.
 * Sozinho no lobby o card some (#285): como no client oficial, a sala só serve com 2 ou mais membros.
 */
export function LobbyChatPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected' || !state.lobby) return null;
  if (state.lobby.members.length < 2) return null;
  if (!state.lobbyChat) return (
    <section aria-label="Chat do grupo" className={cardClass}>
      <h2 className="px-1 text-section font-semibold">Chat do grupo</h2>
      <p className="px-1 text-body text-muted">Aguardando a sala do grupo abrir…</p>
    </section>
  );
  // Trocar de sessão ou de jogador desmonta o formulário e invalida respostas em trânsito.
  return <OpenChat key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} chat={state.lobbyChat} send={send} />;
}

function OpenChat({ state, chat, send }: { state: AgentState; chat: LobbyChat; send: AgentClient['send'] }) {
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
      const result = await send({ type: 'sendLobbyMessage', body });
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
    <section aria-label="Chat do grupo" className={cardClass}>
      <h2 className="px-1 text-section font-semibold">Chat do grupo</h2>
      {messages.length === 0 ? <p className="px-1 text-body text-muted">Nenhuma mensagem ainda.</p>
        : (
          <ol ref={log} onScroll={() => {
            const element = log.current;
            if (element) follow.current = element.scrollHeight - element.scrollTop - element.clientHeight <= 32;
          }} role="log" aria-label="Mensagens do grupo" className="flex max-h-56 flex-col gap-1 overflow-y-auto px-1">
            {messages.map(message => <ChatLine key={message.id} state={state} message={message} />)}
          </ol>
        )}
      <form onSubmit={submit} className="flex gap-2">
        <label htmlFor={inputId} className="sr-only">Mensagem para o grupo</label>
        <input id={inputId} type="text" value={draft} autoComplete="off" placeholder="Mensagem para o grupo"
          onChange={event => setDraft(event.target.value)}
          className="h-9 min-w-0 flex-1 rounded-control border border-control bg-surface px-3 text-body text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent" />
        <button type="submit" disabled={!draft.trim() || sending}
          className="h-9 rounded-control bg-accent px-3 text-body font-semibold text-on-accent disabled:cursor-not-allowed disabled:bg-raised disabled:text-muted">
          {sending ? 'Enviando…' : 'Enviar'}
        </button>
      </form>
      {error && <p role="alert" className="text-body text-danger">{error}</p>}
    </section>
  );
}

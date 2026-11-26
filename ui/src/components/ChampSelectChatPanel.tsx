import { useEffect, useId, useLayoutEffect, useRef, useState, type FormEvent, type KeyboardEvent } from 'react';
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

/** Ícone de balão do chat. */
const ChatIcon = () => (
  <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8"
    strokeLinecap="round" strokeLinejoin="round" className="shrink-0"><path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z" /></svg>
);

/** `true` quando a tecla foi apertada num lugar que já usa o Enter (campo de texto, botão, link, select). */
function usesEnter(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || !!target.closest('input, textarea, select, button, a[href], [role="button"], [role="option"], [role="combobox"]');
}

/** Chat do time na seleção de campeões (#27). Recolhido mostra as últimas mensagens; ao clicar ou apertar Enter, expande para cima (#681). */
export function ChampSelectChatPanel(props: Props) {
  if (!props.chat) return (
    <section aria-label="Chat do time" className="champ-select-chat relative min-w-0 text-ink">
      <div className="champ-select-chat-box flex min-w-0 flex-col gap-2 rounded-card border border-edge bg-surface card p-3">
        <h3 className="sr-only">Chat do time</h3>
        <p className="text-muted">Aguardando a sala do chat abrir…</p>
        <div className="mt-auto flex gap-2">
          <input aria-label="Mensagem para o time" disabled placeholder="Mensagem para o time"
            className="h-9 min-w-0 flex-1 rounded-control border border-control bg-surface px-3 text-body text-ink disabled:opacity-50" />
          <button disabled className="h-9 rounded-control px-3 text-body font-semibold disabled:cursor-not-allowed disabled:bg-raised disabled:text-muted">Enviar</button>
        </div>
      </div>
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
  const section = useRef<HTMLElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  const [expanded, setExpanded] = useState(false);
  // Altura do card recolhido, mantida enquanto o chat expandido flutua por cima da tela.
  const [height, setHeight] = useState<number | null>(null);
  const messages = chat.messages;
  const lastId = messages.at(-1)?.id;

  function expand() {
    if (expanded) return;
    setHeight(section.current?.getBoundingClientRect().height || null);
    follow.current = true;
    setExpanded(true);
  }
  function collapse(focusToggle: boolean) {
    setExpanded(false);
    setHeight(null);
    if (focusToggle) requestAnimationFrame(() => toggle.current?.focus());
  }

  // Ao expandir, o campo ganha o foco e o histórico desce até a última mensagem.
  useLayoutEffect(() => {
    if (!expanded) return;
    input.current?.focus();
    const element = log.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [expanded]);

  // Enter fora de um campo de texto abre o chat, como no client oficial.
  useEffect(() => {
    if (expanded) return;
    function onKeyDown(event: globalThis.KeyboardEvent) {
      if (event.key !== 'Enter' || event.defaultPrevented || event.isComposing || event.repeat) return;
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || usesEnter(event.target)) return;
      if (document.querySelector('[aria-modal="true"], dialog[open]')) return;
      event.preventDefault();
      expand();
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  });

  // Clicar fora fecha o chat expandido, sem perder o rascunho.
  useEffect(() => {
    if (!expanded) return;
    function onPointerDown(event: PointerEvent) {
      if (event.target instanceof Node && !section.current?.contains(event.target)) collapse(false);
    }
    document.addEventListener('pointerdown', onPointerDown);
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [expanded]);

  function onInputKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'Escape') { event.preventDefault(); collapse(true); }
    // Enter com o campo vazio fecha, como no client oficial.
    if (event.key === 'Enter' && !draft.trim() && !event.nativeEvent.isComposing) { event.preventDefault(); collapse(true); }
  }

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
    <section ref={section} aria-label="Chat do time" data-expanded={expanded || undefined}
      style={height ? { height } : undefined}
      onBlur={event => {
        // Sair com Tab fecha; um clique dentro do card (no histórico, por exemplo) não.
        const next = event.relatedTarget;
        if (expanded && next instanceof Node && !event.currentTarget.contains(next)) collapse(false);
      }}
      className="champ-select-chat relative min-w-0 text-ink">
      <div onClick={expanded ? undefined : expand}
        className={`champ-select-chat-box flex min-w-0 flex-col gap-2 rounded-card border bg-surface card p-3 ${expanded ? 'border-accent' : 'cursor-pointer border-edge'}`}>
        <h3 className="sr-only">Chat do time</h3>
        {messages.length === 0 ? <p className="text-muted">Nenhuma mensagem ainda.</p>
          : (
            <ol ref={log} onScroll={() => {
              const element = log.current;
              if (element) follow.current = element.scrollHeight - element.scrollTop - element.clientHeight <= 32;
            }} role="log" aria-label="Mensagens do time" className="champ-select-chat-log flex min-h-5 flex-col gap-1 overflow-y-auto pr-1">
              {messages.map(message => (
                <li key={message.id} className="break-words text-body">
                  <span className="text-caption text-muted">{messageTime(message)} </span>
                  <span className={`font-semibold ${message.fromMe ? 'text-accent-ink' : 'text-ink'}`}>{senderName(champSelect, message)}: </span>
                  <span className="whitespace-pre-wrap">{message.body}</span>
                </li>
              ))}
            </ol>
          )}
        {expanded ? (
          <form onSubmit={submit} className="mt-auto flex gap-2">
            <label htmlFor={inputId} className="sr-only">Mensagem para o time</label>
            <input ref={input} id={inputId} type="text" value={draft} autoComplete="off" placeholder="Mensagem para o time"
              onChange={event => setDraft(event.target.value)} onKeyDown={onInputKeyDown}
              className="h-9 min-w-0 flex-1 rounded-control border border-control bg-surface px-3 text-body text-ink disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent" />
            <button type="submit" disabled={!draft.trim() || sending}
              className="h-9 rounded-control bg-accent px-3 text-body font-semibold text-on-accent disabled:cursor-not-allowed disabled:bg-raised disabled:text-muted">
              {sending ? 'Enviando…' : 'Enviar'}
            </button>
          </form>
        ) : (
          <button ref={toggle} type="button" aria-expanded="false" aria-label="Abrir o chat do time"
            className="mt-auto flex h-9 min-w-0 shrink-0 items-center gap-2 rounded-control border border-control bg-surface px-3 text-left text-body text-muted hover:border-accent focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent">
            <ChatIcon />
            <span className="min-w-0 truncate">
              {draft.trim() ? <span className="text-ink">{draft}</span>
                : <>Clique ou <kbd className="rounded-[0.25rem] bg-raised px-1.5 py-0.5 font-sans text-caption text-ink">Enter</kbd> para conversar</>}
            </span>
          </button>
        )}
        {error && <p role="alert" className="text-danger">{error}</p>}
      </div>
    </section>
  );
}

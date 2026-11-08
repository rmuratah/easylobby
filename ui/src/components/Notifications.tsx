import { useCallback, useEffect, useState } from 'react';
import type { ConnectionSnapshot } from '../agent/client';
import { invitationText, matchFoundPending, updateMessageNotices, type MessageNotice } from '../agent/notifications';
import type { AgentState } from '../bindings/AgentState';
import { sound } from '../agent/sound';
import { cx } from './base/cx';

/** Tempo que um aviso de mensagem fica na tela sem mensagem nova. */
export const MESSAGE_NOTICE_MS = 8_000;

type Props = {
  connection: ConnectionSnapshot;
  /**
   * Seleção de campeões: só os avisos de mensagem aparecem, compactos, para
   * não cobrir os times. Os convites continuam na coluna e no botão dela (#132).
   */
  quiet?: boolean;
  /** Com a coluna de amigos aberta, o convite já aparece nela; o aviso flutuante só repetiria. */
  sidebarOpen?: boolean;
  /** Conversa aberta e à vista na tela de Conversas: as mensagens dela não viram aviso. */
  openConversationId?: string | null;
  /** Clique no aviso de mensagem: abre a conversa na tela de Conversas (#251). */
  onOpenConversation?: (id: string) => void;
};

/**
 * Avisos de mensagem, convite e partida encontrada (#33), no canto da janela.
 * Só avisam: responder ao convite, à partida ou à mensagem continua nas telas
 * de cada assunto (#32, #20 e #31). O aviso de mensagem abre a conversa.
 */
export function Notifications({ connection, quiet = false, sidebarOpen = false, openConversationId = null, onOpenConversation }: Props) {
  const state = connection.status === 'connected' && connection.state?.lcu.state === 'connected' ? connection.state : null;
  const [seen, setSeen] = useState<AgentState | null>(null);
  const [messages, setMessages] = useState<MessageNotice[]>([]);
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  // Compara com o estado anterior durante o render, como a documentação do
  // React recomenda para derivar estado de props que mudam.
  if (state !== seen) {
    setSeen(state);
    setMessages(current => updateMessageNotices(current, seen, state));
  }
  const dismissMessage = useCallback(
    (id: string) => setMessages(current => current.filter(n => n.conversationId !== id)), []);
  const invitations = quiet || sidebarOpen ? [] : state?.receivedInvitations.filter(i => !dismissed.has(i.invitationId)) ?? [];
  const matchFound = state !== null && matchFoundPending(state);
  useMatchFoundSound(matchFound);
  const notices = messages.filter(n => n.conversationId !== openConversationId);
  if (!matchFound && invitations.length === 0 && notices.length === 0) return null;
  return (
    <section aria-label="Notificações"
      className={cx('fixed right-4 top-[8.5rem] z-50 flex max-w-[calc(100vw-2rem)] flex-col gap-2', quiet ? 'w-64' : 'w-80')}>
      {matchFound && (
        <div role="alert" className="rounded-card border-2 border-accent bg-surface p-3 text-ink shadow-[0_8px_24px_-12px_var(--el-overlay)]">
          <p className="font-semibold text-accent-ink">Partida encontrada!</p>
          <p className="text-caption text-muted">Aceite ou recuse no painel da partida.</p>
        </div>
      )}
      {invitations.map(invitation => (
        <Notice key={invitation.invitationId} title="Convite para jogar" text={invitationText(invitation)}
          onDismiss={() => setDismissed(current => new Set(current).add(invitation.invitationId))} />
      ))}
      {notices.map(notice => <MessageToast key={notice.conversationId} notice={notice} compact={quiet} onDismiss={dismissMessage}
        onOpen={onOpenConversation && (() => { dismissMessage(notice.conversationId); onOpenConversation(notice.conversationId); })} />)}
    </section>
  );
}

/**
 * Toca o som uma vez quando a partida é encontrada. O navegador só libera o
 * áudio depois de um clique ou tecla; o primeiro já basta para os próximos.
 */
function useMatchFoundSound(matchFound: boolean) {
  useEffect(() => {
    const unlock = () => sound.unlock();
    window.addEventListener('pointerdown', unlock, { once: true });
    window.addEventListener('keydown', unlock, { once: true });
    return () => {
      window.removeEventListener('pointerdown', unlock);
      window.removeEventListener('keydown', unlock);
    };
  }, []);
  useEffect(() => {
    if (matchFound) sound.playMatchFound();
  }, [matchFound]);
}

function MessageToast({ notice, compact, onDismiss, onOpen }: {
  notice: MessageNotice; compact: boolean; onDismiss: (id: string) => void; onOpen?: () => void;
}) {
  const { conversationId, lastMessageId } = notice;
  useEffect(() => {
    const timer = setTimeout(() => onDismiss(conversationId), MESSAGE_NOTICE_MS);
    return () => clearTimeout(timer);
    // Reinicia o tempo a cada mensagem nova (lastMessageId muda).
  }, [conversationId, lastMessageId, onDismiss]);
  const title = notice.count > 1 ? `${notice.count} mensagens de ${notice.from}` : `Mensagem de ${notice.from}`;
  return <Notice title={title} text={notice.body} compact={compact} onDismiss={() => onDismiss(conversationId)}
    onOpen={onOpen} openLabel={`Abrir conversa com ${notice.from}`} />;
}

function Notice({ title, text, compact = false, onDismiss, onOpen, openLabel }: {
  title: string; text: string; compact?: boolean; onDismiss: () => void; onOpen?: () => void; openLabel?: string;
}) {
  const body = <>
    <p className="font-semibold break-words">{title}</p>
    <p className={cx('break-words text-caption text-muted', compact ? 'line-clamp-1' : 'line-clamp-2')}>{text}</p>
  </>;
  return (
    <div role="status" className={cx('flex items-start gap-2 rounded-card border border-line bg-surface text-ink shadow-[0_8px_24px_-12px_var(--el-overlay)]',
      compact ? 'px-3 py-2' : 'p-3')}>
      {onOpen
        ? <button type="button" onClick={onOpen} aria-label={openLabel} title={openLabel}
            className="min-w-0 flex-1 rounded-control-sm text-left hover:opacity-80 focus-visible:outline-2 focus-visible:outline-accent">{body}</button>
        : <div className="min-w-0 flex-1">{body}</div>}
      <button type="button" aria-label={`Fechar aviso: ${title}`}
        className="flex h-6 w-6 shrink-0 items-center justify-center rounded-control-sm text-muted hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-accent"
        onClick={onDismiss}>×</button>
    </div>
  );
}

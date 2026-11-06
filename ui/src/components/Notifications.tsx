import { useCallback, useEffect, useState } from 'react';
import type { ConnectionSnapshot } from '../agent/client';
import { invitationText, matchFoundPending, updateMessageNotices, type MessageNotice } from '../agent/notifications';
import type { AgentState } from '../bindings/AgentState';
import { sound } from '../agent/sound';

/** Tempo que um aviso de mensagem fica na tela sem mensagem nova. */
export const MESSAGE_NOTICE_MS = 8_000;

type Props = { connection: ConnectionSnapshot };

/**
 * Avisos de mensagem, convite e partida encontrada (#33), no canto da janela.
 * Só avisam: responder ao convite, à partida ou à mensagem continua nas telas
 * de cada assunto (#32, #20 e #31).
 */
export function Notifications({ connection }: Props) {
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
  const invitations = state?.receivedInvitations.filter(i => !dismissed.has(i.invitationId)) ?? [];
  const matchFound = state !== null && matchFoundPending(state);
  useMatchFoundSound(matchFound);
  if (!matchFound && invitations.length === 0 && messages.length === 0) return null;
  return (
    <section aria-label="Notificações"
      className="fixed right-4 top-4 z-50 flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2">
      {matchFound && (
        <div role="alert" className="rounded border border-sky-400 bg-slate-900 p-3 shadow-lg">
          <p className="font-semibold">Partida encontrada!</p>
          <p className="text-sm text-slate-300">Aceite ou recuse no painel da partida.</p>
        </div>
      )}
      {invitations.map(invitation => (
        <Notice key={invitation.invitationId} title="Convite para jogar" text={invitationText(invitation)}
          onDismiss={() => setDismissed(current => new Set(current).add(invitation.invitationId))} />
      ))}
      {messages.map(notice => <MessageToast key={notice.conversationId} notice={notice} onDismiss={dismissMessage} />)}
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

function MessageToast({ notice, onDismiss }: { notice: MessageNotice; onDismiss: (id: string) => void }) {
  const { conversationId, lastMessageId } = notice;
  useEffect(() => {
    const timer = setTimeout(() => onDismiss(conversationId), MESSAGE_NOTICE_MS);
    return () => clearTimeout(timer);
    // Reinicia o tempo a cada mensagem nova (lastMessageId muda).
  }, [conversationId, lastMessageId, onDismiss]);
  const title = notice.count > 1 ? `${notice.count} mensagens de ${notice.from}` : `Mensagem de ${notice.from}`;
  return <Notice title={title} text={notice.body} onDismiss={() => onDismiss(conversationId)} />;
}

function Notice({ title, text, onDismiss }: { title: string; text: string; onDismiss: () => void }) {
  return (
    <div role="status" className="flex items-start gap-2 rounded border border-slate-600 bg-slate-900 p-3 shadow-lg">
      <div className="min-w-0 flex-1">
        <p className="font-semibold break-words">{title}</p>
        <p className="line-clamp-2 break-words text-sm text-slate-300">{text}</p>
      </div>
      <button type="button" aria-label={`Fechar aviso: ${title}`} className="rounded px-2 text-slate-400 hover:text-slate-100"
        onClick={onDismiss}>×</button>
    </div>
  );
}

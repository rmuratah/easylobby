import type { AgentState } from '../bindings/AgentState';
import type { Conversation } from '../bindings/Conversation';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import { queueNames } from './queues';

/** Aviso de mensagem nova numa conversa 1:1 (#33). Um por conversa. */
export type MessageNotice = {
  conversationId: string;
  from: string;
  /** Texto da última mensagem recebida. */
  body: string;
  /** Quantas mensagens chegaram desde que o aviso apareceu. */
  count: number;
  /** Id da última mensagem; muda a cada mensagem nova e reinicia o tempo do aviso. */
  lastMessageId: string;
};

export function conversationName(conversation: Conversation): string {
  if (!conversation.gameName) return 'Um amigo';
  return conversation.tagLine ? `${conversation.gameName}#${conversation.tagLine}` : conversation.gameName;
}

export function invitationText(invitation: ReceivedInvitation): string {
  const from = invitation.fromSummonerName || 'Um amigo';
  const queue = invitation.queue ? queueNames[invitation.queue] : 'um lobby';
  return `${from} convidou você para ${queue}`;
}

/** Partida encontrada esperando a resposta do jogador. */
export function matchFoundPending(state: AgentState): boolean {
  const check = state.readyCheck;
  return state.gameflowPhase === 'ReadyCheck' && check !== null && check.state === 'InProgress' && check.playerResponse === 'None';
}

/**
 * Atualiza os avisos de mensagem de `prev` para `next`. Sem estado anterior
 * (primeiro `hello`, reconexão, outra sessão do LoL ou outro invocador) o
 * histórico que já existe não vira aviso. Um aviso some quando a conversa é
 * lida (`unreadCount` zera) ou fechada; mensagens de uma conversa já lida,
 * como a que está aberta na tela, não avisam.
 */
export function updateMessageNotices(
  notices: MessageNotice[], prev: AgentState | null, next: AgentState | null,
): MessageNotice[] {
  if (!next) return notices.length ? [] : notices;
  if (!prev || !sameSession(prev, next)) return notices.length ? [] : notices;
  if (prev.conversations === next.conversations) return notices;
  const byId = new Map(next.conversations.map(c => [c.id, c]));
  let result = notices.filter(n => (byId.get(n.conversationId)?.unreadCount ?? 0) > 0);
  for (const conversation of next.conversations) {
    if (conversation.unreadCount === 0) continue;
    const before = prev.conversations.find(c => c.id === conversation.id);
    const known = new Set(before?.messages.map(m => m.id));
    const fresh = conversation.messages.filter(m => !m.fromMe && !known.has(m.id));
    const last = fresh.at(-1);
    if (!last) continue;
    const current = result.find(n => n.conversationId === conversation.id);
    const notice: MessageNotice = {
      conversationId: conversation.id, from: conversationName(conversation), body: last.body,
      count: (current?.count ?? 0) + fresh.length, lastMessageId: last.id,
    };
    result = current ? result.map(n => n === current ? notice : n) : [...result, notice];
  }
  return result.length === notices.length && result.every((n, i) => n === notices[i]) ? notices : result;
}

function sameSession(a: AgentState, b: AgentState): boolean {
  const session = (s: AgentState) => s.lcu.state === 'connected' ? s.lcu.session : null;
  return session(a) !== null && session(a) === session(b) && a.summoner?.puuid === b.summoner?.puuid;
}

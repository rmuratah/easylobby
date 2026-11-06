import type { ChatMessage } from '../bindings/ChatMessage';
import type { Conversation } from '../bindings/Conversation';
import type { Friend } from '../bindings/Friend';
import { friendDisplayName } from './friends';

/**
 * Nome do amigo de uma conversa: o Riot ID que veio com a conversa ou, sem
 * ele, o nome do amigo na lista de amigos.
 */
export function conversationName(conversation: Pick<Conversation, 'id' | 'gameName' | 'tagLine'>, friends: Friend[]): string {
  if (conversation.gameName) return conversation.tagLine ? `${conversation.gameName}#${conversation.tagLine}` : conversation.gameName;
  const friend = friends.find(f => f.id === conversation.id);
  return friend ? friendDisplayName(friend) : 'Amigo';
}

function lastTime(conversation: Conversation): number {
  const last = conversation.messages.at(-1);
  const time = last ? Date.parse(last.timestamp) : NaN;
  return Number.isNaN(time) ? -Infinity : time;
}

/**
 * Conversas da mais recente para a mais antiga, pela última mensagem. O
 * agente não manda as conversas em ordem. Conversas sem mensagem vão para o
 * fim, em ordem de nome.
 */
export function sortConversations(conversations: Conversation[], friends: Friend[]): Conversation[] {
  return [...conversations].sort((a, b) =>
    lastTime(b) - lastTime(a) || conversationName(a, friends).localeCompare(conversationName(b, friends), 'pt-BR'));
}

/** Total de mensagens não lidas, para o título do painel. */
export function totalUnread(conversations: Conversation[]): number {
  return conversations.reduce((total, c) => total + c.unreadCount, 0);
}

/** Hora da mensagem no fuso do computador (`14:05`), ou vazio com horário inválido. */
export function messageTime(message: ChatMessage): string {
  const date = new Date(message.timestamp);
  if (Number.isNaN(date.getTime())) return '';
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
}

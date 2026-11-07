import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { CommandResult } from '../bindings/CommandResult';
import type { Conversation } from '../bindings/Conversation';
import type { Friend } from '../bindings/Friend';
import { testConversation, testFriend } from '../test/events';
import { makeState } from '../test/state';
import { ChatButton, ChatPanel } from './ChatPanel';

const connectedWith = (conversations: Conversation[], friends: Friend[] = []): ConnectionSnapshot =>
  ({ status: 'connected', error: null, state: makeState({ conversations, friends }) });
const bia: Conversation = {
  id: 'bia@br1.pvp.net', gameName: 'Bia', tagLine: 'BR1', unreadCount: 0,
  messages: [{ id: 'b1', body: 'gg', fromMe: true, timestamp: '2026-11-05T03:00:00.000Z' }],
};

function deferred() {
  let resolve!: (result: CommandResult) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<CommandResult>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function Harness({ connection, send, initial = null }: { connection: ConnectionSnapshot; send: AgentClient['send']; initial?: string | null }) {
  const [activeId, setActiveId] = useState(initial);
  return <ChatPanel connection={connection} send={send} activeId={activeId} onActiveChange={setActiveId} />;
}
const okSend = () => vi.fn<AgentClient['send']>(() => Promise.resolve({ status: 'ok' }));
const commands = (send: ReturnType<typeof okSend>) => send.mock.calls.map(([command]) => command);

describe('ChatPanel', () => {
  it('não mostra nada sem conexão com o LoL nem sem conversa aberta', () => {
    const { container, rerender } = render(<Harness connection={{ status: 'reconnecting', error: null, state: null }} send={okSend()} />);
    expect(container).toBeEmptyDOMElement();
    rerender(<Harness connection={connectedWith([testConversation, bia])} send={okSend()} />);
    expect(container).toBeEmptyDOMElement();
    expect(screen.queryByRole('region', { name: 'Conversas' })).toBeNull();
  });

  it('abre a conversa, marca como ativa e desmarca ao fechar', () => {
    const send = okSend();
    const { container } = render(<Harness connection={connectedWith([testConversation])} send={send} initial={testConversation.id} />);
    const view = within(screen.getByRole('region', { name: 'Conversa com Amigo' }));
    expect(view.getByRole('log', { name: 'Mensagens' })).toHaveTextContent('Amigo: bora?');
    expect(commands(send)).toEqual([{ type: 'setActiveConversation', conversationId: testConversation.id }]);
    fireEvent.click(view.getByRole('button', { name: 'Fechar conversa' }));
    expect(container).toBeEmptyDOMElement();
    expect(commands(send).at(-1)).toEqual({ type: 'setActiveConversation', conversationId: null });
  });

  it('marca as não lidas no balão do amigo', () => {
    const onOpen = vi.fn();
    const friend = { ...testFriend, id: testConversation.id };
    const { rerender } = render(<ChatButton friend={friend} onOpen={onOpen} />);
    expect(screen.getByRole('button', { name: 'Conversar com Amigo' }).querySelector('[data-unread]')).toBeNull();
    rerender(<ChatButton friend={friend} unread={2} onOpen={onOpen} />);
    const button = screen.getByRole('button', { name: 'Conversar com Amigo, 2 não lidas' });
    expect(button.querySelector('[data-unread]')).not.toBeNull();
    fireEvent.click(button);
    expect(onOpen).toHaveBeenCalledWith(testConversation.id);
  });

  it('manda a mensagem, limpa o campo e mostra a resposta pelo evento', async () => {
    const pending = deferred();
    const send = vi.fn<AgentClient['send']>(command => command.type === 'sendChatMessage' ? pending.promise : Promise.resolve({ status: 'ok' }));
    const { rerender } = render(<Harness connection={connectedWith([testConversation])} send={send} initial={testConversation.id} />);
    const input = screen.getByRole('textbox', { name: 'Mensagem para Amigo' });
    expect(screen.getByRole('button', { name: 'Enviar' })).toBeDisabled();
    fireEvent.change(input, { target: { value: '  bora!  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Enviar' }));
    fireEvent.submit(input);
    expect(send.mock.calls.filter(([c]) => c.type === 'sendChatMessage')).toEqual([
      [{ type: 'sendChatMessage', conversationId: testConversation.id, body: 'bora!' }],
    ]);
    expect(screen.getByRole('button', { name: 'Enviando…' })).toBeDisabled();
    await act(async () => { pending.resolve({ status: 'ok' }); });
    expect(input).toHaveValue('');
    const updated = { ...testConversation, messages: [...testConversation.messages,
      { id: 'm2', body: 'bora!', fromMe: true, timestamp: '2026-11-05T01:14:00.000Z' }] };
    rerender(<Harness connection={connectedWith([updated])} send={send} initial={testConversation.id} />);
    expect(screen.getByRole('log')).toHaveTextContent('Você: bora!');
  });

  it('mantém o texto e mostra o erro quando o envio falha', async () => {
    const send = vi.fn<AgentClient['send']>(command => command.type === 'sendChatMessage'
      ? Promise.resolve({ status: 'error', code: 'lcu', message: 'amigo offline' }) : Promise.resolve({ status: 'ok' }));
    render(<Harness connection={connectedWith([testConversation])} send={send} initial={testConversation.id} />);
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'oi' } });
    await act(async () => { fireEvent.submit(input); });
    expect(screen.getByRole('alert')).toHaveTextContent('Não foi possível enviar a mensagem: amigo offline');
    expect(input).toHaveValue('oi');
  });

  it('abre conversa nova com um amigo e só marca como ativa quando ela existir', () => {
    const send = okSend();
    const friend = { ...testFriend, id: 'novo@br1.pvp.net', gameName: 'Novo' };
    const { rerender } = render(<Harness connection={connectedWith([], [friend])} send={send} initial={friend.id} />);
    expect(screen.getByRole('region', { name: 'Conversa com Novo' })).toHaveTextContent('Nenhuma mensagem nesta sessão');
    expect(send).not.toHaveBeenCalled();
    const created = { ...testConversation, id: friend.id, gameName: 'Novo', unreadCount: 0 };
    rerender(<Harness connection={connectedWith([created], [friend])} send={send} initial={friend.id} />);
    expect(commands(send)).toEqual([{ type: 'setActiveConversation', conversationId: friend.id }]);
  });

  it('fecha quando a conversa some e não é de um amigo', () => {
    const { container, rerender } = render(<Harness connection={connectedWith([testConversation])} send={okSend()} initial={testConversation.id} />);
    expect(screen.getByRole('region', { name: 'Conversa com Amigo' })).toBeInTheDocument();
    rerender(<Harness connection={connectedWith([])} send={okSend()} initial={testConversation.id} />);
    expect(container).toBeEmptyDOMElement();
  });
});

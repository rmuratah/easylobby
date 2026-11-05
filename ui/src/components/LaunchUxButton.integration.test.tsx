import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import { AGENT_URL } from '../agent/client';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';

class Socket {
  static OPEN = 1;
  static instances: Socket[] = [];
  readyState = 1;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  send = vi.fn();
  close = vi.fn();
  constructor(readonly url: string) { Socket.instances.push(this); }
  receive(message: ServerMessage) { this.onmessage?.({ data: JSON.stringify(message) }); }
}
const hello: ServerMessage = {
  type: 'hello', protocol: 1, agentVersion: '0.1.0',
  state: { lcu: { state: 'connected', session: 1 }, summoner: null, gameflowPhase: null, readyCheck: null, lobby: null, matchmaking: null, champSelect: null, receivedInvitations: [] },
};
beforeEach(() => {
  vi.useFakeTimers();
  Socket.instances = [];
  vi.stubGlobal('WebSocket', Socket);
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('reutiliza o único /ws do App, recebe sucesso e recupera uma abertura interrompida', async () => {
  const { unmount } = render(<App />);
  const socket = Socket.instances[0];
  expect(Socket.instances).toHaveLength(1);
  expect(socket.url).toBe(AGENT_URL);
  act(() => { socket.receive(hello); });
  const button = () => screen.getByRole('button', { name: /client oficial/ });
  fireEvent.click(button());
  const request: ClientMessage = JSON.parse(socket.send.mock.calls[0][0]);
  expect(request.command).toEqual({ type: 'launchUx' });
  await act(async () => { socket.receive({ type: 'response', id: request.id, result: { status: 'ok' } }); });
  expect(screen.getByText('Abertura do client oficial confirmada.')).toBeInTheDocument();
  fireEvent.click(button());
  await act(async () => { socket.onclose?.(); });
  expect(button()).toBeDisabled();
  expect(screen.queryByText('Abertura do client oficial confirmada.')).not.toBeInTheDocument();
  act(() => { vi.advanceTimersByTime(1000); });
  const next = Socket.instances[1];
  act(() => { next.receive(hello); });
  expect(button()).toBeEnabled();
  expect(next.send).not.toHaveBeenCalled();
  fireEvent.click(button());
  await act(async () => { vi.advanceTimersByTime(10000); });
  expect(screen.getByRole('alert')).toHaveTextContent('Não foi possível confirmar a abertura');
  expect(button()).toBeEnabled();
  unmount();
});

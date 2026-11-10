import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { RunePages } from '../bindings/RunePages';
import type { ServerMessage } from '../bindings/ServerMessage';
import { champSelectState } from '../test/champSelect';
import { testFullRuneCatalog, testRunePage } from '../test/runes';

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
const socket = () => Socket.instances.at(-1)!;
const panel = () => within(screen.getByRole('region', { name: 'Páginas de runas' }));
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const pages: RunePages = {
  pages: [testRunePage, { ...testRunePage, id: 11, name: 'Top' }], currentPageId: 10, canCreatePage: true,
};

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

describe('páginas de runas pelo /ws', () => {
  it('aplica uma página e mostra a página em uso confirmada pelo agente', async () => {
    render(<App />);
    act(() => socket().receive({
      type: 'hello', protocol: 1, agentVersion: '0.1.0',
      state: { ...champSelectState, runePages: pages, runeCatalog: testFullRuneCatalog },
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Editar runas' }));
    // O mesmo editor do Jogo Dinâmico (#420), com a página em uso.
    const select = () => panel().getByRole('combobox', { name: 'Página de runas' });
    expect(select()).toHaveValue('10');
    act(() => { fireEvent.change(select(), { target: { value: '11' } }); });
    expect(request().command).toEqual({ type: 'selectRunePage', pageId: 11 });
    fireEvent.click(screen.getByRole('button', { name: 'Voltar para Seleção de campeões' }));
    fireEvent.click(screen.getByRole('button', { name: 'Editar runas' }));
    expect(select()).toBeDisabled();
    expect(Socket.instances).toHaveLength(1);
    // Só muda quando o agente publica as páginas de novo.
    expect(select()).toHaveValue('10');
    act(() => socket().receive({ type: 'runePages', runePages: { ...pages, currentPageId: 11 } }));
    await act(async () => socket().receive({ type: 'response', id: request().id, result: { status: 'ok' } }));
    expect(select()).toHaveValue('11');
    expect(select()).toBeEnabled();
  });
});

import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import type { CommandResult } from '../bindings/CommandResult';
import { LaunchUxButton } from './LaunchUxButton';

const connected: ConnectionSnapshot = {
  status: 'connected', error: null,
  state: { lcu: { state: 'connected', session: 1 }, summoner: null, gameflowPhase: null, readyCheck: null, lobby: null, matchmaking: null, champSelect: null, friends: [], runeCatalog: null, runePages: null, conversations: [] },
};
function deferred() {
  let resolve!: (result: CommandResult) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<CommandResult>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}
const button = () => screen.getByRole('button', { name: /client oficial/ });

describe('LaunchUxButton', () => {
  it('envia launchUx uma vez, indica execução e confirma sucesso somente após a resposta', async () => {
    const request = deferred();
    const send = vi.fn(() => request.promise);
    render(<LaunchUxButton connection={connected} send={send} />);
    expect(button()).toBeEnabled();
    act(() => { fireEvent.click(button()); fireEvent.click(button()); });
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'launchUx' });
    expect(button()).toBeDisabled();
    expect(button()).toHaveAttribute('aria-busy', 'true');
    expect(screen.getByRole('status')).toHaveTextContent('Abrindo');
    await act(async () => { request.resolve({ status: 'ok' }); });
    expect(screen.getByRole('status')).toHaveTextContent('Abertura do client oficial confirmada');
    expect(button()).toBeEnabled();
  });

  it.each([
    ['notConnected', 'Aguarde a reconexão'],
    ['lcu', 'Não foi possível abrir'],
  ] as const)('trata o erro %s e permite nova tentativa', async (code, message) => {
    const send = vi.fn<() => Promise<CommandResult>>()
      .mockResolvedValueOnce({ status: 'error', code, message: 'erro da LCU' })
      .mockResolvedValueOnce({ status: 'ok' });
    render(<LaunchUxButton connection={connected} send={send} />);
    fireEvent.click(button());
    expect(await screen.findByRole('alert')).toHaveTextContent(message);
    expect(button()).toBeEnabled();
    fireEvent.click(button());
    expect(await screen.findByText('Abertura do client oficial confirmada.')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(send).toHaveBeenCalledTimes(2);
  });

  it.each(['Conexão com o agente encerrada.', 'O agente não respondeu ao comando.', 'Não foi possível enviar o comando.'])('trata rejeição: %s', async error => {
    const send = vi.fn().mockRejectedValue(new Error(error));
    render(<LaunchUxButton connection={connected} send={send} />);
    fireEvent.click(button());
    expect(await screen.findByRole('alert')).toHaveTextContent('Não foi possível confirmar a abertura');
    expect(button()).toBeEnabled();
  });

  const unavailable: ConnectionSnapshot[] = [
    ...(['connecting', 'reconnecting', 'incompatible'] as const).map(status => ({ ...connected, status })),
    { ...connected, state: null },
    ...([
      { state: 'starting' },
      { state: 'disconnected', reason: 'fechado' },
      { state: 'retrying', attempt: 1, retryInMs: 1000, clientNotRunning: true, error: 'fechado' },
    ] as const).map(lcu => ({ ...connected, state: { ...connected.state!, lcu } })),
  ];
  it.each(unavailable)('bloqueia sem conexão ativa: %j', connection => {
    const send = vi.fn();
    render(<LaunchUxButton connection={connection} send={send} />);
    expect(button()).toBeDisabled();
    expect(button()).toHaveAccessibleDescription(/Aguarde a conexão/);
    fireEvent.click(button());
    expect(send).not.toHaveBeenCalled();
  });

  it.each(['agente', 'LoL', 'sessão'] as const)('descarta resposta antiga após mudança de %s e permite nova tentativa', async source => {
    const old = deferred();
    const current = deferred();
    const send = vi.fn().mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
    const { rerender } = render(<LaunchUxButton connection={connected} send={send} />);
    fireEvent.click(button());
    const next: ConnectionSnapshot = source === 'agente'
      ? { status: 'reconnecting', state: null, error: 'conexão perdida' }
      : { ...connected, state: { ...connected.state!, lcu: source === 'LoL'
        ? { state: 'disconnected', reason: 'fechado' } : { state: 'connected', session: 2 } } };
    rerender(<LaunchUxButton connection={next} send={send} />);
    if (source !== 'sessão') {
      expect(button()).toBeDisabled();
      expect(button()).toHaveAccessibleDescription(/confira o client oficial/);
    }
    rerender(<LaunchUxButton connection={connected} send={send} />);
    expect(button()).toBeEnabled();
    fireEvent.click(button());
    await act(async () => { old.resolve({ status: 'ok' }); });
    expect(button()).toBeDisabled();
    expect(screen.getByRole('status')).toHaveTextContent('Abrindo');
    await act(async () => { current.resolve({ status: 'ok' }); });
    expect(button()).toBeEnabled();
    expect(screen.getByRole('status')).toHaveTextContent('confirmada');
  });

  it('consome rejeição tardia após desmontar', async () => {
    const request = deferred();
    const { unmount } = render(<LaunchUxButton connection={connected} send={() => request.promise} />);
    fireEvent.click(button());
    unmount();
    await act(async () => { request.reject(new Error('conexão encerrada')); });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});

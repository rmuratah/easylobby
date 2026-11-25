import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import { parseMessage } from '../agent/protocol';
import type { Vanguard } from '../bindings/Vanguard';
import { makeState } from '../test/state';
import { VanguardBanner } from './VanguardNotice';

const send = vi.fn();
const connection = (vanguard: Vanguard | null): ConnectionSnapshot => ({
  status: 'connected', error: null, state: makeState({ vanguard }),
});

describe('aviso de Vanguard desconectado (#579)', () => {
  it('não mostra nada com o Vanguard conectado ou sem resposta da LCU', () => {
    const { container, rerender } = render(
      <VanguardBanner connection={connection({ connected: true, state: 'CONNECTED', status: 0 })} send={send} />);
    expect(container).toBeEmptyDOMElement();
    rerender(<VanguardBanner connection={connection(null)} send={send} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('avisa que a partida vira esquiva e mostra o estado da LCU', () => {
    render(<VanguardBanner connection={connection({ connected: false, state: 'NOT_CONNECTED', status: 3 })} send={send} />);
    const banner = screen.getByRole('alert', { name: 'Vanguard desconectado' });
    expect(banner).toHaveTextContent('O Vanguard não está conectado');
    expect(banner).toHaveTextContent('esquiva');
    expect(banner).toHaveTextContent('NOT_CONNECTED (código 3)');
  });

  it('não mostra nada com o LoL fechado', () => {
    const closed: ConnectionSnapshot = {
      status: 'connected', error: null,
      state: makeState({ lcu: { state: 'disconnected', reason: 'fechou' }, vanguard: { connected: false, state: 'X', status: null } }),
    };
    const { container } = render(<VanguardBanner connection={closed} send={send} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('aceita a mensagem do agente e recusa um formato errado', () => {
    expect(parseMessage(JSON.stringify({ type: 'vanguard', vanguard: { connected: false, state: 'NOT_CONNECTED', status: null } })))
      .toEqual({ type: 'vanguard', vanguard: { connected: false, state: 'NOT_CONNECTED', status: null } });
    expect(parseMessage(JSON.stringify({ type: 'vanguard', vanguard: null }))).toEqual({ type: 'vanguard', vanguard: null });
    expect(() => parseMessage(JSON.stringify({ type: 'vanguard', vanguard: { connected: 'sim' } }))).toThrow();
  });

  it('reabre o LoL pelo agente e mostra o erro da seleção', async () => {
    const restart = vi.fn()
      .mockResolvedValueOnce({ status: 'error', code: 'invalidCommand', message: 'seleção' })
      .mockResolvedValueOnce({ status: 'ok' });
    render(<VanguardBanner connection={connection({ connected: false, state: 'NOT_CONNECTED', status: 3 })} send={restart} />);
    fireEvent.click(screen.getByRole('button', { name: 'Reabrir o LoL' }));
    expect(restart).toHaveBeenCalledWith({ type: 'restartLeague' });
    expect(await screen.findByText('Não dá para reabrir o LoL com partida encontrada, na seleção de campeões nem na partida.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Reabrir o LoL' }));
    expect(await screen.findByRole('status')).toHaveTextContent('O LoL está abrindo de novo.');
  });

  it('no VAN 216, manda esperar até o horário e não oferece reabrir o LoL (#668)', () => {
    const refused = connection({ connected: false, state: 'ERROR', status: 216 });
    const { rerender } = render(<VanguardBanner connection={refused} send={send} waitUntil={new Date(2026, 10, 24, 21, 52, 18).getTime()} />);
    const banner = screen.getByRole('alert', { name: 'Vanguard desconectado' });
    // O horário arredonda para o minuto de cima.
    expect(screen.getByRole('heading', { name: 'Espere até as 21:53 para abrir o LoL de novo' })).toBeInTheDocument();
    expect(banner).toHaveTextContent('erro VAN 216');
    expect(banner).toHaveTextContent('reabrir agora não resolve');
    expect(banner).toHaveTextContent('ERROR (código 216)');
    expect(screen.queryByRole('button', { name: 'Reabrir o LoL' })).not.toBeInTheDocument();
    // Antes de o app guardar a hora da recusa.
    rerender(<VanguardBanner connection={refused} send={send} waitUntil={null} />);
    expect(screen.getByRole('heading', { name: 'Espere 30 minutos para abrir o LoL de novo' })).toBeInTheDocument();
  });

  it('com outro código, continua oferecendo reabrir o LoL (#668)', () => {
    render(<VanguardBanner connection={connection({ connected: false, state: 'ERROR', status: 7003 })} send={send}
      waitUntil={new Date(2026, 10, 24, 21, 52, 18).getTime()} />);
    expect(screen.getByRole('heading', { name: 'O Vanguard não está conectado' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Reabrir o LoL' })).toBeInTheDocument();
  });
});

import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import { parseMessage } from '../agent/protocol';
import type { Vanguard } from '../bindings/Vanguard';
import { makeState } from '../test/state';
import { VanguardBanner } from './VanguardNotice';

const connection = (vanguard: Vanguard | null): ConnectionSnapshot => ({
  status: 'connected', error: null, state: makeState({ vanguard }),
});

describe('aviso de Vanguard desconectado (#579)', () => {
  it('não mostra nada com o Vanguard conectado ou sem resposta da LCU', () => {
    const { container, rerender } = render(
      <VanguardBanner connection={connection({ connected: true, state: 'CONNECTED', status: 0 })} />);
    expect(container).toBeEmptyDOMElement();
    rerender(<VanguardBanner connection={connection(null)} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('avisa que a partida vira esquiva e mostra o estado da LCU', () => {
    render(<VanguardBanner connection={connection({ connected: false, state: 'NOT_CONNECTED', status: 3 })} />);
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
    const { container } = render(<VanguardBanner connection={closed} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('aceita a mensagem do agente e recusa um formato errado', () => {
    expect(parseMessage(JSON.stringify({ type: 'vanguard', vanguard: { connected: false, state: 'NOT_CONNECTED', status: null } })))
      .toEqual({ type: 'vanguard', vanguard: { connected: false, state: 'NOT_CONNECTED', status: null } });
    expect(parseMessage(JSON.stringify({ type: 'vanguard', vanguard: null }))).toEqual({ type: 'vanguard', vanguard: null });
    expect(() => parseMessage(JSON.stringify({ type: 'vanguard', vanguard: { connected: 'sim' } }))).toThrow();
  });
});

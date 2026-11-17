import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import { parseMessage } from '../agent/protocol';
import { makeState } from '../test/state';
import { LcuSlowBanner } from './LcuSlowBanner';

const connection = (lcuSlow: boolean): ConnectionSnapshot => ({
  status: 'connected', error: null, state: makeState({ lcuSlow }),
});
const send = vi.fn();

describe('aviso de LoL demorando para responder (#580)', () => {
  it('avisa enquanto o LoL demora para responder', () => {
    render(<LcuSlowBanner connection={connection(true)} send={send} />);
    const banner = screen.getByRole('status', { name: 'LoL demorando para responder' });
    expect(banner).toHaveTextContent('O LoL está demorando para responder');
    expect(screen.getByRole('button', { name: /client oficial/i })).toBeInTheDocument();
  });

  it('some quando o LoL responde', () => {
    const { container } = render(<LcuSlowBanner connection={connection(false)} send={send} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('aceita a mensagem lcuSlow do agente', () => {
    expect(parseMessage(JSON.stringify({ type: 'lcuSlow', slow: true }))).toEqual({ type: 'lcuSlow', slow: true });
    expect(() => parseMessage(JSON.stringify({ type: 'lcuSlow', slow: 'sim' }))).toThrow();
  });
});

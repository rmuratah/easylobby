import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import { parseMessage } from '../agent/protocol';
import type { UnavailableFeature } from '../bindings/UnavailableFeature';
import { makeState } from '../test/state';
import { CompatibilityBanner, FeatureGate } from './CompatibilityNotice';

const lobbyOff: UnavailableFeature = {
  feature: 'lobby', path: '/lol-lobby/v2/lobby', problem: 'missing',
  detail: 'GET /lol-lobby/v2/lobby respondeu 404: Invalid URI format',
};
const socialOff: UnavailableFeature = {
  feature: 'social', path: '/lol-chat/v1/friends', problem: 'unexpectedFormat',
  detail: 'esperava uma lista, veio um texto',
};
const connection = (unavailableFeatures: UnavailableFeature[]): ConnectionSnapshot => ({
  status: 'connected', error: null, state: makeState({ unavailableFeatures }),
});
const send = vi.fn();

describe('modo de compatibilidade (#36)', () => {
  it('não mostra nada com tudo funcionando', () => {
    const { container } = render(<CompatibilityBanner connection={connection([])} send={send} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('lista as funções desligadas, oferece o client oficial e esconde no Entendi', () => {
    render(<CompatibilityBanner connection={connection([lobbyOff, socialOff])} send={send} />);
    const banner = screen.getByRole('alert', { name: 'Modo de compatibilidade' });
    expect(banner).toHaveTextContent('Lobby e fila e Amigos e conversas');
    expect(banner).toHaveTextContent('/lol-lobby/v2/lobby');
    expect(screen.getByRole('button', { name: 'Abrir client oficial' })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: 'Entendi' }));
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('troca só a tela da função desligada pelo aviso', () => {
    const off = connection([lobbyOff]);
    render(<>
      <FeatureGate connection={off} send={send} feature="lobby"><p>tela do lobby</p></FeatureGate>
      <FeatureGate connection={off} send={send} feature="lobby" quiet><p>fila</p></FeatureGate>
      <FeatureGate connection={off} send={send} feature="champSelect"><p>tela da seleção</p></FeatureGate>
    </>);
    expect(screen.queryByText('tela do lobby')).toBeNull();
    expect(screen.queryByText('fila')).toBeNull();
    expect(screen.getByText('tela da seleção')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('Lobby e fila: função desligada');
    expect(screen.getByRole('button', { name: 'Abrir client oficial' })).toBeInTheDocument();
  });

  it('valida a mensagem do agente', () => {
    expect(parseMessage(JSON.stringify({ type: 'unavailableFeatures', unavailableFeatures: [lobbyOff] })))
      .toEqual({ type: 'unavailableFeatures', unavailableFeatures: [lobbyOff] });
    expect(() => parseMessage(JSON.stringify({ type: 'unavailableFeatures', unavailableFeatures: [{ ...lobbyOff, feature: 'loja' }] })))
      .toThrow();
  });
});

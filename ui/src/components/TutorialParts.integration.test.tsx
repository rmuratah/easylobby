import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { ServerMessage } from '../bindings/ServerMessage';
import type { TutorialPart } from '../bindings/TutorialPart';
import { testState } from '../test/lobby';

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
const panel = () => within(screen.getByRole('region', { name: 'Lobby' }));
const radio = (name: string) => panel().getByRole('radio', { name: new RegExp(`^${name}`) });
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
function openTutorial() {
  fireEvent.click(panel().getByRole('tab', { name: 'Treinamento' }));
  expect(radio('Tutorial')).toBeChecked();
}
const part = (queueId: number, n: number, completed: boolean, available: boolean): TutorialPart => ({ queueId, part: n, completed, available });
beforeEach(() => { configure({ defaultHidden: true }); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

describe('partes do Tutorial (#228)', () => {
  it('mostra as três partes, com cadeado na que falta liberar, e inicia a escolhida', () => {
    render(<App />);
    hello({ tutorial: [part(2000, 1, true, true), part(2010, 2, false, true), part(2020, 3, false, false)] });
    openTutorial();
    const parte1 = radio('Parte 1');
    const parte2 = radio('Parte 2');
    const parte3 = radio('Parte 3');
    expect(parte1).toHaveAccessibleName(/Bem-vindo\(a\) ao League of Legends/);
    expect(parte2).toHaveAccessibleName(/Fortalecimento/);
    expect(parte3).toHaveAccessibleName(/Comprando equipamentos/);
    expect(parte3).toHaveAccessibleDescription(/Conclua a partida de tutorial anterior para habilitar/);
    expect(parte2).toHaveAccessibleDescription(/150 de EXP/);
    // Começa na primeira liberada que falta concluir.
    expect(parte2).toBeChecked();
    expect(parte3).toBeDisabled();
    expect(panel().getByText('Concluída')).toBeInTheDocument();
    // A concluída dá para jogar de novo.
    fireEvent.click(parte1);
    expect(parte1).toBeChecked();
    fireEvent.click(panel().getByRole('button', { name: 'Iniciar' }));
    expect(request().command).toEqual({ type: 'startTutorial', queueId: 2000 });
  });

  it('com a progressão nova, a parte liberada fica disponível', () => {
    render(<App />);
    hello({ tutorial: [part(2000, 1, false, true), part(2010, 2, false, false), part(2020, 3, false, false)] });
    openTutorial();
    expect(radio('Parte 2')).toBeDisabled();
    act(() => socket().receive({ type: 'tutorial', tutorial: [part(2000, 1, true, true), part(2010, 2, false, true), part(2020, 3, false, false)] }));
    expect(radio('Parte 2')).toBeEnabled();
    expect(radio('Parte 2')).toBeChecked();
  });

  it('sem o progresso, deixa escolher qualquer parte', () => {
    render(<App />);
    hello({ tutorial: null });
    openTutorial();
    for (const n of [1, 2, 3]) expect(radio(`Parte ${n}`)).toBeEnabled();
    fireEvent.click(radio('Parte 3'));
    fireEvent.click(panel().getByRole('button', { name: 'Iniciar' }));
    expect(request().command).toEqual({ type: 'startTutorial', queueId: 2020 });
  });
});

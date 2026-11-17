import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LolUpdate } from '../bindings/LolUpdate';
import { DONE_NOTICE_MS, LolUpdateBanner } from './LolUpdateBanner';

const downloading: LolUpdate = { stage: 'downloading', percent: 42, blocksQueue: true };
const banner = () => screen.queryByRole('status', { name: 'Atualização do League of Legends' });

beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); });

describe('aviso de atualização do LoL (#575)', () => {
  it('não mostra nada sem atualização', () => {
    render(<LolUpdateBanner update={null} />);
    expect(banner()).toBeNull();
  });

  it('mostra a etapa, a porcentagem e que a fila espera', () => {
    render(<LolUpdateBanner update={downloading} />);
    expect(banner()).toHaveTextContent('Atualizando o League of Legends: 42%');
    expect(banner()).toHaveTextContent('A busca de partida fica travada até o patch terminar');
    expect(screen.getByRole('progressbar', { name: 'Progresso da atualização' })).toHaveAttribute('aria-valuenow', '42');
  });

  it('sem porcentagem, não mostra a barra', () => {
    render(<LolUpdateBanner update={{ stage: 'checking', percent: null, blocksQueue: true }} />);
    expect(banner()).toHaveTextContent('Preparando a atualização do League of Legends');
    expect(screen.queryByRole('progressbar')).toBeNull();
  });

  it('atualização esperando o grupo e a partida', () => {
    render(<LolUpdateBanner update={{ stage: 'pending', percent: null, blocksQueue: false }} />);
    expect(banner()).toHaveTextContent('Atualização do League of Legends disponível');
    expect(banner()).toHaveTextContent('quando você sair do grupo e da partida');
  });

  it('avisa que terminou e some depois de alguns segundos', () => {
    const { rerender } = render(<LolUpdateBanner update={downloading} />);
    rerender(<LolUpdateBanner update={null} />);
    expect(banner()).toHaveTextContent('Atualização do League of Legends concluída');
    act(() => { vi.advanceTimersByTime(DONE_NOTICE_MS); });
    expect(banner()).toBeNull();
  });

  it('uma atualização que só estava esperando não vira "concluída" ao sumir', () => {
    const { rerender } = render(<LolUpdateBanner update={{ stage: 'pending', percent: null, blocksQueue: false }} />);
    rerender(<LolUpdateBanner update={null} />);
    expect(banner()).toBeNull();
  });
});

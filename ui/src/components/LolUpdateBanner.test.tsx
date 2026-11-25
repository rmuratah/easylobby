import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { LolUpdate } from '../bindings/LolUpdate';
import { LolUpdateBanner } from './LolUpdateBanner';

const downloading: LolUpdate = { stage: 'downloading', percent: 42, blocksQueue: true };
const banner = () => screen.queryByRole('status', { name: 'Atualização do League of Legends' });

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

  it('some quando o patch termina, sem aviso de concluída (#666)', () => {
    const { rerender } = render(<LolUpdateBanner update={downloading} />);
    rerender(<LolUpdateBanner update={null} />);
    expect(banner()).toBeNull();
  });
});

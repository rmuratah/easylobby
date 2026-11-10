import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { makeState } from '../test/state';
import { testLiveGame } from '../test/liveGame';
import { LiveGamePanel } from './LiveGamePanel';

const state = makeState({
  championCatalog: { champions: [
    { id: 103, name: 'Ahri', alias: 'Ahri', roles: [], positions: [] },
    { id: 26, name: 'Zilean', alias: 'Zilean', roles: [], positions: [] },
  ] },
});

describe('LiveGamePanel', () => {
  afterEach(() => vi.useRealTimers());

  it('mostra a fila, o mapa e as duas equipes', () => {
    render(<LiveGamePanel state={state} game={testLiveGame} />);
    expect(screen.getByRole('heading', { name: /Ranqueada Solo\/Duo/ })).toHaveTextContent("Summoner's Rift");
    expect(screen.getByLabelText('Tempo de jogo')).toHaveTextContent('2:05');
    const [blue, red] = screen.getAllByRole('table');
    expect(within(blue).getByText('Equipe azul')).toBeInTheDocument();
    expect(within(blue).getByRole('caption')).toHaveTextContent('Elo médio: Ouro II');
    expect(within(red).getByRole('caption')).toHaveTextContent('Equipe vermelhaElo médio: —');
  });

  it('mostra elo, vitórias, campeão, temporada passada e etiquetas', () => {
    render(<LiveGamePanel state={state} game={testLiveGame} />);
    const me = screen.getByRole('row', { name: 'Victor#BR1, Ahri' });
    expect(me).toHaveTextContent('45 PDL');
    expect(me).toHaveTextContent('60% (50 partidas)');
    expect(me).toHaveTextContent('75%4 partidas');
    expect(within(me).getAllByText('Ouro II').length).toBeGreaterThan(0);
    expect(within(me).getByText('Prata I')).toBeInTheDocument();
    expect(within(me).getByText('Meio')).toBeInTheDocument();
    const rival = screen.getByRole('row', { name: 'Rival#NA1, Ahri' });
    expect(rival).toHaveTextContent('4 derrotas seguidas');
  });

  it('jogador anônimo e jogador carregando não quebram a tela', () => {
    render(<LiveGamePanel state={state} game={testLiveGame} />);
    const hidden = screen.getByRole('row', { name: 'Jogador anônimo, Zilean' });
    expect(hidden).not.toHaveTextContent('PDL');
    const loading = screen.getAllByRole('row').find(r => r.textContent?.includes('Carregando…'));
    expect(loading).toBeDefined();
  });

  it('abre o perfil de outro jogador, mas não o do próprio', () => {
    const onOpenProfile = vi.fn();
    render(<LiveGamePanel state={state} game={testLiveGame} onOpenProfile={onOpenProfile} />);
    fireEvent.click(screen.getByRole('button', { name: 'Ver perfil de Rival#NA1' }));
    expect(onOpenProfile).toHaveBeenCalledWith('p3');
    expect(screen.queryByRole('button', { name: /Victor/ })).toBeNull();
    expect(screen.queryByRole('button', { name: /anônimo/ })).toBeNull();
  });

  it('o relógio anda sozinho e espera o jogo carregar', () => {
    vi.useFakeTimers();
    const { rerender } = render(<LiveGamePanel state={state} game={{ ...testLiveGame, gameTimeSecs: null }} />);
    expect(screen.getByLabelText('Tempo de jogo')).toHaveTextContent('Carregando…');
    rerender(<LiveGamePanel state={state} game={testLiveGame} />);
    act(() => vi.advanceTimersByTime(3000));
    expect(screen.getByLabelText('Tempo de jogo')).toHaveTextContent('2:08');
  });
});

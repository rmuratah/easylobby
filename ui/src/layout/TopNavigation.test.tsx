import { fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { ToolbarSettings, TopNavigation } from './TopNavigation';

const soundProps = { sound: { enabled: true, volume: 0.7 }, onSoundChange: () => {} };

describe('TopNavigation', () => {
  it('mantém a ordem, o destino ativo e o Jogar separado no centro', () => {
    const onNavigate = vi.fn();
    const { container } = render(<TopNavigation active="home" flowLabel="Grupo" flowButtonRef={createRef()} onNavigate={onNavigate} />);
    const navigation = screen.getByRole('navigation', { name: 'Funcionalidades' });
    expect(screen.getByRole('heading', { name: 'easylobby' })).toHaveAttribute('data-tauri-drag-region');
    const buttons = Array.from(navigation.querySelectorAll('button'));
    expect(buttons.map(button => button.getAttribute('aria-label'))).toEqual([
      'Início', 'Perfil', 'Coleção', 'Conversas', 'Loja — em breve', 'Grupo',
    ]);
    expect(screen.getByRole('button', { name: 'Início' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', { name: 'Loja — em breve' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Loja — em breve' }));
    expect(onNavigate).not.toHaveBeenCalled();
    for (const button of buttons) expect(button).not.toHaveAttribute('data-tauri-drag-region');
    // O Jogar fica sozinho no espaço do centro da barra, fora do grupo das telas.
    const play = screen.getByRole('button', { name: 'Grupo' });
    expect(play).toHaveClass('app-play-button');
    expect(play.parentElement).toHaveClass('app-play-slot');
    expect(container.querySelector('.app-toolbar-start')).not.toContainElement(play);
    fireEvent.click(play);
    expect(onNavigate).toHaveBeenCalledWith('flow');
    fireEvent.click(screen.getByRole('button', { name: 'Perfil' }));
    expect(onNavigate).toHaveBeenCalledWith('profile');
    fireEvent.click(screen.getByRole('button', { name: 'Coleção' }));
    expect(onNavigate).toHaveBeenCalledWith('collection');
    fireEvent.click(screen.getByRole('button', { name: 'Conversas' }));
    expect(onNavigate).toHaveBeenCalledWith('conversations');
  });

  it('marca o Jogar como página aberta na tela do fluxo', () => {
    const ref = createRef<HTMLButtonElement>();
    render(<TopNavigation active="flow" flowLabel="Jogar" flowButtonRef={ref} onNavigate={vi.fn()} />);
    const play = screen.getByRole('button', { name: 'Jogar' });
    expect(play).toHaveAttribute('aria-current', 'page');
    expect(play).toHaveAttribute('title', 'Jogar');
    expect(ref.current).toBe(play);
    expect(screen.getByRole('button', { name: 'Início' })).not.toHaveAttribute('aria-current');
  });

  it('marca as não lidas no botão Conversas', () => {
    const { rerender } = render(<TopNavigation active="home" flowLabel="Jogar" flowButtonRef={createRef()} onNavigate={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Conversas' }).querySelector('[data-unread]')).toBeNull();
    rerender(<TopNavigation active="home" flowLabel="Jogar" flowButtonRef={createRef()} unread={3} onNavigate={vi.fn()} />);
    expect(screen.getByRole('button', { name: 'Conversas, 3 não lidas' }).querySelector('[data-unread]')).not.toBeNull();
  });

  it('tem o logo curto e o inteiro, e o CSS escolhe pela largura da janela', () => {
    const { container } = render(<TopNavigation active="home" flowLabel="Jogar" flowButtonRef={createRef()} onNavigate={vi.fn()} />);
    expect(container.querySelector('.app-logo-short')).toHaveTextContent('el');
    expect(container.querySelector('.app-logo-full')).toHaveTextContent('easylobby');
  });
});

describe('ToolbarSettings', () => {
  it('fica só com o botão de configurações, e o tema, só claro e escuro, dentro dele', () => {
    const onThemeChange = vi.fn();
    render(<ToolbarSettings theme="escuro" onThemeChange={onThemeChange} {...soundProps} />);
    expect(screen.getAllByRole('button')).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Configurações' }));
    const menu = screen.getByRole('dialog', { name: 'Configurações' });
    expect(menu).toHaveTextContent('Aparência');
    expect(screen.queryByRole('button', { name: 'Sistema' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Tema escuro' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: 'Tema claro' }));
    expect(onThemeChange).toHaveBeenCalledWith('claro');
  });
});

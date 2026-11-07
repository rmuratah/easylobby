import { fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { SideNavigation } from './SideNavigation';

describe('SideNavigation', () => {
  it('mantém a ordem, o destino ativo e a reserva de configurações', () => {
    const onNavigate = vi.fn();
    const { container } = render(<SideNavigation active="home" flowLabel="Grupo" flowButtonRef={createRef()}
      theme="claro" onNavigate={onNavigate} onThemeChange={vi.fn()} />);
    const navigation = screen.getByRole('complementary', { name: 'Navegação do aplicativo' });
    expect(screen.getByRole('heading', { name: 'easylobby' })).toHaveAttribute('data-tauri-drag-region');
    const buttons = Array.from(navigation.querySelectorAll('button'));
    expect(buttons.map(button => button.getAttribute('aria-label') ?? button.textContent)).toEqual([
      'Início', 'Grupo', 'Perfil', 'Tema claro', 'Tema escuro', 'Configurações — em breve',
    ]);
    expect(screen.getByRole('button', { name: 'Início' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', { name: 'Configurações — em breve' })).toBeDisabled();
    for (const button of buttons) expect(button).not.toHaveAttribute('data-tauri-drag-region');
    fireEvent.click(screen.getByRole('button', { name: 'Grupo' }));
    expect(onNavigate).toHaveBeenCalledWith('flow');
    const nav = container.querySelector('nav')!;
    expect(Array.from(nav.children).map(child => child.tagName === 'BUTTON' ? child.getAttribute('aria-label') : 'divisor'))
      .toEqual(['Início', 'Grupo', 'divisor', 'Perfil']);
    fireEvent.click(screen.getByRole('button', { name: 'Perfil' }));
    expect(onNavigate).toHaveBeenCalledWith('profile');
  });

  it('não tem botão de recolher: fica estreita e expande pelo CSS ao passar o mouse', () => {
    render(<SideNavigation active="home" flowLabel="Jogar" flowButtonRef={createRef()}
      theme="escuro" onNavigate={vi.fn()} onThemeChange={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /navegação/ })).toBeNull();
    const navigation = screen.getByRole('complementary', { name: 'Navegação do aplicativo' });
    expect(navigation.parentElement).toHaveClass('app-navigation-slot');
    expect(screen.getByRole('button', { name: 'Jogar' })).toHaveAttribute('title', 'Jogar');
    expect(navigation.querySelector('.app-logo-short')).toHaveTextContent('el');
    expect(navigation.querySelector('.app-logo-full')).toHaveTextContent('easylobby');
  });

  it('troca o tema só entre claro e escuro', () => {
    const onThemeChange = vi.fn();
    render(<SideNavigation active="flow" flowLabel="Grupo" flowButtonRef={createRef()}
      theme="escuro" onNavigate={vi.fn()} onThemeChange={onThemeChange} />);
    expect(screen.queryByRole('button', { name: 'Sistema' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Tema escuro' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: 'Tema claro' }));
    expect(onThemeChange).toHaveBeenCalledWith('claro');
  });
});

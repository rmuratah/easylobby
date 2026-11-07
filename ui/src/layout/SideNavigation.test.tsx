import { fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { NAVIGATION_STORAGE_KEY } from './navigation';
import { SideNavigation } from './SideNavigation';

const renderNavigation = () => render(<SideNavigation active="home" flowLabel="Lobby" flowButtonRef={createRef()}
  theme="claro" onNavigate={vi.fn()} onThemeChange={vi.fn()} />);

beforeEach(() => localStorage.clear());

describe('SideNavigation', () => {
  it('mantém a ordem, o destino ativo e a reserva de configurações', () => {
    const onNavigate = vi.fn();
    const { container } = render(<SideNavigation active="home" flowLabel="Lobby" flowButtonRef={createRef()}
      theme="claro" onNavigate={onNavigate} onThemeChange={vi.fn()} />);
    const navigation = screen.getByRole('complementary', { name: 'Navegação do aplicativo' });
    expect(screen.getByRole('heading', { name: 'easylobby' })).toHaveAttribute('data-tauri-drag-region');
    const buttons = Array.from(navigation.querySelectorAll('button'));
    expect(buttons.map(button => button.getAttribute('aria-label') ?? button.textContent)).toEqual([
      'Lobby', 'Início', 'Recolher navegação', 'Tema claro', 'Tema escuro', 'Configurações — em breve',
    ]);
    expect(screen.getByRole('button', { name: 'Início' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', { name: 'Configurações — em breve' })).toBeDisabled();
    for (const button of buttons) expect(button).not.toHaveAttribute('data-tauri-drag-region');
    fireEvent.click(screen.getByRole('button', { name: 'Lobby' }));
    expect(onNavigate).toHaveBeenCalledWith('flow');
    expect(container.querySelector('nav')).toBeInTheDocument();
  });

  it('troca o tema só entre claro e escuro', () => {
    const onThemeChange = vi.fn();
    render(<SideNavigation active="flow" flowLabel="Lobby" flowButtonRef={createRef()}
      theme="escuro" onNavigate={vi.fn()} onThemeChange={onThemeChange} />);
    expect(screen.queryByRole('button', { name: 'Sistema' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Tema escuro' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: 'Tema claro' }));
    expect(onThemeChange).toHaveBeenCalledWith('claro');
  });

  it('recolhe para só os ícones, expande de novo e lembra a escolha', () => {
    const { unmount } = renderNavigation();
    const navigation = screen.getByRole('complementary', { name: 'Navegação do aplicativo' });
    expect(screen.getByText('Início')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Recolher navegação' }));
    expect(navigation).toHaveClass('is-collapsed');
    expect(screen.queryByText('Início')).toBeNull();
    expect(screen.queryByText('Lobby')).toBeNull();
    expect(screen.getByRole('button', { name: 'Início' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', { name: 'Lobby' })).toHaveAttribute('title', 'Lobby');
    expect(screen.getByRole('heading', { name: 'easylobby' })).toBeInTheDocument();
    expect(localStorage.getItem(NAVIGATION_STORAGE_KEY)).toBe('recolhida');

    unmount();
    renderNavigation();
    const expand = screen.getByRole('button', { name: 'Expandir navegação' });
    expect(expand).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(expand);
    expect(screen.getByText('Início')).toBeInTheDocument();
    expect(localStorage.getItem(NAVIGATION_STORAGE_KEY)).toBe('expandida');
  });
});

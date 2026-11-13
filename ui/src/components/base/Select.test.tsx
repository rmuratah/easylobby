import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { Select, type SelectOption } from './Select';

const options: SelectOption<string>[] = [
  { value: 'top', label: 'Topo' },
  { value: 'jungle', label: 'Selva' },
  { value: 'middle', label: 'Meio', disabled: true },
  { value: 'bottom', label: 'Atirador' },
  { value: 'utility', label: 'Suporte', hint: 'Prioritária' },
];

function Harness({ onChange = () => {}, initial = 'jungle', disabled = false }: {
  onChange?: (value: string) => void; initial?: string | null; disabled?: boolean;
}) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <label htmlFor="rota">Rota</label>
      <Select id="rota" value={value} options={options} disabled={disabled} placeholder="Escolha a rota"
        onChange={next => { setValue(next); onChange(next); }} />
      <button type="button">Fora</button>
    </>
  );
}

const combobox = () => screen.getByRole('combobox', { name: 'Rota' });
const active = () => document.getElementById(combobox().getAttribute('aria-activedescendant') ?? '');

describe('Select (#479)', () => {
  it('mostra a opção escolhida, abre pelo clique e escolhe outra', () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    expect(combobox()).toHaveTextContent('Selva');
    expect(combobox()).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(combobox());
    expect(combobox()).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Selva' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('option', { name: /Suporte/ })).toHaveTextContent('Prioritária');
    fireEvent.click(screen.getByRole('option', { name: 'Atirador' }));
    expect(onChange).toHaveBeenCalledWith('bottom');
    expect(combobox()).toHaveTextContent('Atirador');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(combobox()).toHaveFocus();
  });

  it('opção desabilitada não se escolhe nem pelo clique nem pelas setas', () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    fireEvent.click(combobox());
    fireEvent.click(screen.getByRole('option', { name: 'Meio' }));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByRole('option', { name: 'Meio' })).toHaveAttribute('aria-disabled', 'true');
    // Da Selva, a seta para baixo pula o Meio.
    fireEvent.keyDown(combobox(), { key: 'ArrowDown' });
    expect(active()).toHaveTextContent('Atirador');
  });

  it('funciona só pelo teclado: setas, Home/End, Enter, Esc e Tab', () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    combobox().focus();
    fireEvent.keyDown(combobox(), { key: 'ArrowDown' });
    expect(combobox()).toHaveAttribute('aria-expanded', 'true');
    expect(active()).toHaveTextContent('Selva');
    fireEvent.keyDown(combobox(), { key: 'End' });
    expect(active()).toHaveTextContent('Suporte');
    fireEvent.keyDown(combobox(), { key: 'ArrowDown' });
    expect(active()).toHaveTextContent('Suporte');
    fireEvent.keyDown(combobox(), { key: 'Home' });
    expect(active()).toHaveTextContent('Topo');
    fireEvent.keyDown(combobox(), { key: 'ArrowUp' });
    expect(active()).toHaveTextContent('Topo');
    // Esc fecha sem trocar.
    fireEvent.keyDown(combobox(), { key: 'Escape' });
    expect(combobox()).toHaveAttribute('aria-expanded', 'false');
    expect(onChange).not.toHaveBeenCalled();
    // Enter abre; seta e Enter escolhem.
    fireEvent.keyDown(combobox(), { key: 'Enter' });
    fireEvent.keyDown(combobox(), { key: 'ArrowUp' });
    fireEvent.keyDown(combobox(), { key: 'Enter' });
    expect(onChange).toHaveBeenLastCalledWith('top');
    expect(combobox()).toHaveAttribute('aria-expanded', 'false');
    // Tab escolhe a opção ativa e segue o foco.
    fireEvent.keyDown(combobox(), { key: ' ' });
    fireEvent.keyDown(combobox(), { key: 'End' });
    fireEvent.keyDown(combobox(), { key: 'Tab' });
    expect(onChange).toHaveBeenLastCalledWith('utility');
  });

  it('digitar pula para a opção que começa com as letras', () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    fireEvent.keyDown(combobox(), { key: 'a' });
    expect(active()).toHaveTextContent('Atirador');
    fireEvent.keyDown(combobox(), { key: 's' });
    // "as" não existe; continua na Atirador.
    expect(active()).toHaveTextContent('Atirador');
  });

  it('clique fora fecha sem trocar', () => {
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    fireEvent.click(combobox());
    fireEvent.pointerDown(screen.getByRole('button', { name: 'Fora' }));
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('sem valor mostra o texto de espera; desabilitado não abre', () => {
    render(<Harness initial={null} disabled />);
    expect(combobox()).toHaveTextContent('Escolha a rota');
    expect(combobox()).toBeDisabled();
    fireEvent.click(combobox());
    fireEvent.keyDown(combobox(), { key: 'ArrowDown' });
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });
});

import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModeSelect } from './ModeSelect';
import { mapIcons } from '../agent/customGame';

function modeSelect(summonerName?: string) {
  return <ModeSelect queues={[]} canCreate pending={false} restriction="" descriptionId="ajuda" feedback={null}
    onCreate={vi.fn()} onStart={vi.fn()} summonerName={summonerName} onCreateCustom={vi.fn()} />;
}
const nameField = () => screen.getByRole('textbox', { name: 'Nome' });

describe('nome sugerido da personalizada (#278)', () => {
  it('usa o invocador que chega depois de a tela montar', () => {
    const { rerender } = render(modeSelect());
    fireEvent.click(screen.getByRole('tab', { name: 'Personalizada' }));
    expect(nameField()).toHaveValue('Partida personalizada');
    rerender(modeSelect('Victor'));
    expect(nameField()).toHaveValue('Partida de Victor');
  });

  it('não troca o nome que o jogador editou', () => {
    const { rerender } = render(modeSelect());
    fireEvent.click(screen.getByRole('tab', { name: 'Personalizada' }));
    fireEvent.change(nameField(), { target: { value: 'Treino do time' } });
    rerender(modeSelect('Victor'));
    expect(nameField()).toHaveValue('Treino do time');
  });
});

describe('rodapé da personalizada (#332)', () => {
  it('mostra o ícone do mapa escolhido ao lado do Confirmar', () => {
    const { container } = render(modeSelect('Victor'));
    fireEvent.click(screen.getByRole('tab', { name: 'Personalizada' }));
    const footerIcon = () => container.querySelector('footer img')?.getAttribute('src');
    expect(footerIcon()).toBe(mapIcons.summonersRift);
    fireEvent.click(screen.getByRole('radio', { name: /Mapa aleatório/ }));
    expect(footerIcon()).toBe(mapIcons.howlingAbyss);
    fireEvent.click(screen.getByRole('radio', { name: /Rift Clássico/ }));
    expect(footerIcon()).toBe(mapIcons.classicRift);
  });
});

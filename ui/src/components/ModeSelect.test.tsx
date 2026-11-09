import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModeSelect } from './ModeSelect';

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

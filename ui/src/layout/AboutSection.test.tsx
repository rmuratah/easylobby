import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { RIOT_LEGAL_NOTICE } from './AboutSection';
import { SoundMenu } from './SoundMenu';

it('mostra o aviso legal da Riot na seção Sobre das Configurações', () => {
  render(<SoundMenu settings={{ enabled: true, volume: 0.7 }} onChange={() => {}} icon={null} />);
  fireEvent.click(screen.getByRole('button', { name: 'Configurações' }));
  expect(screen.getByRole('heading', { name: 'Sobre' })).toBeTruthy();
  expect(screen.getByText(RIOT_LEGAL_NOTICE)).toBeTruthy();
});

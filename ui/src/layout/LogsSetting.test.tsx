import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { LogsSetting } from './LogsSetting';

it('abre a pasta de logs pelo shell', () => {
  const api = { openFolder: vi.fn(() => Promise.resolve()) };
  render(<LogsSetting api={api} />);
  fireEvent.click(screen.getByRole('button', { name: 'Abrir pasta de logs' }));
  expect(api.openFolder).toHaveBeenCalledTimes(1);
});

it('avisa quando não consegue abrir a pasta', async () => {
  const api = { openFolder: () => Promise.reject(new Error('explorer')) };
  render(<LogsSetting api={api} />);
  fireEvent.click(screen.getByRole('button', { name: 'Abrir pasta de logs' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Não deu para abrir a pasta agora.');
});

it('some fora do app desktop', () => {
  const { container } = render(<LogsSetting api={null} />);
  expect(container).toBeEmptyDOMElement();
});

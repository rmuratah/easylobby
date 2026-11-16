import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import type { OpenWithLeague } from '../agent/openWithLeague';
import { OpenWithLeagueSetting } from './OpenWithLeagueSetting';

function fakeApi(initial: boolean): OpenWithLeague {
  let value = initial;
  return {
    isEnabled: vi.fn(() => Promise.resolve(value)),
    setEnabled: vi.fn((enabled: boolean) => { value = enabled; return Promise.resolve(value); }),
  };
}

it('mostra a opção ligada e desliga pelo shell', async () => {
  const api = fakeApi(true);
  render(<OpenWithLeagueSetting api={api} />);
  const toggle = await screen.findByRole('switch', { name: 'Abrir junto com o LoL' });
  expect(toggle).toBeChecked();
  fireEvent.click(toggle);
  expect(api.setEnabled).toHaveBeenCalledWith(false);
  await vi.waitFor(() => expect(toggle).not.toBeChecked());
  expect(api.isEnabled).toHaveBeenCalledTimes(1);
});

it('avisa quando o shell não consegue mudar a opção', async () => {
  const api: OpenWithLeague = {
    isEnabled: () => Promise.resolve(false),
    setEnabled: () => Promise.reject(new Error('registro')),
  };
  render(<OpenWithLeagueSetting api={api} />);
  fireEvent.click(await screen.findByRole('switch', { name: 'Abrir junto com o LoL' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Não deu para mudar essa opção agora.');
});

it('some fora do app desktop', () => {
  const { container } = render(<OpenWithLeagueSetting api={null} />);
  expect(container).toBeEmptyDOMElement();
});

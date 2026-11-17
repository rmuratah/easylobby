import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import type { ErrorReports } from '../agent/errorReports';
import { ErrorReportsSetting } from './ErrorReportsSetting';

function fakeApi(initial: boolean | null): ErrorReports {
  let value = initial;
  return {
    isEnabled: vi.fn(() => Promise.resolve(value)),
    setEnabled: vi.fn((enabled: boolean) => { value = enabled; return Promise.resolve(enabled); }),
  };
}

it('mostra a opção ligada, com o aviso, e desliga pelo shell', async () => {
  const api = fakeApi(true);
  render(<ErrorReportsSetting api={api} />);
  const toggle = await screen.findByRole('switch', { name: 'Enviar erros para os devs' });
  expect(toggle).toBeChecked();
  expect(screen.getByText(/Nunca vão seu nick, senhas, chaves nem o chat/)).toBeInTheDocument();
  fireEvent.click(toggle);
  expect(api.setEnabled).toHaveBeenCalledWith(false);
  await vi.waitFor(() => expect(toggle).not.toBeChecked());
});

it('avisa quando o shell não consegue mudar a opção', async () => {
  const api: ErrorReports = {
    isEnabled: () => Promise.resolve(false),
    setEnabled: () => Promise.reject(new Error('disco')),
  };
  render(<ErrorReportsSetting api={api} />);
  fireEvent.click(await screen.findByRole('switch', { name: 'Enviar erros para os devs' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Não deu para mudar essa opção agora.');
});

it('some quando a build não envia erros', async () => {
  const api = fakeApi(null);
  const { container } = render(<ErrorReportsSetting api={api} />);
  await vi.waitFor(() => expect(api.isEnabled).toHaveBeenCalled());
  expect(container).toBeEmptyDOMElement();
});

it('some fora do app desktop', () => {
  const { container } = render(<ErrorReportsSetting api={null} />);
  expect(container).toBeEmptyDOMElement();
});

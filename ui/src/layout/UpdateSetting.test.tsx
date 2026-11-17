import { act, fireEvent, render, renderHook, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { canInstallNow, type AppUpdate, type UpdateInfo } from '../agent/appUpdate';
import { CHECK_INTERVAL_MS, useAppUpdate } from '../agent/useAppUpdate';
import { UpdateSetting } from './UpdateSetting';

const info: UpdateInfo = { currentVersion: '0.1.0', version: '0.2.0', notes: 'Pós-jogo novo.' };

function fakeApi(found: UpdateInfo | null, install: () => Promise<void> = () => new Promise(() => {})): AppUpdate {
  return {
    version: vi.fn(() => Promise.resolve('0.1.0')),
    check: vi.fn(() => Promise.resolve(found)),
    install: vi.fn(install),
  };
}

function Harness({ api, canInstall = true }: { api: AppUpdate | null; canInstall?: boolean }) {
  return <UpdateSetting update={useAppUpdate(api)} canInstall={canInstall} />;
}

afterEach(() => vi.useRealTimers());

it('mostra a versão nova e instala pelo shell', async () => {
  const api = fakeApi(info);
  render(<Harness api={api} />);
  expect(await screen.findByText('Versão 0.2.0 disponível.')).toBeInTheDocument();
  expect(screen.getByText('Versão instalada: 0.1.0')).toBeInTheDocument();
  expect(screen.getByText('Pós-jogo novo.')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Atualizar e reiniciar' }));
  expect(api.install).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('button', { name: 'Instalando…' })).toBeDisabled();
});

it('não deixa instalar na fila, na seleção ou na partida', async () => {
  render(<Harness api={fakeApi(info)} canInstall={false} />);
  expect(await screen.findByRole('button', { name: 'Atualizar e reiniciar' })).toBeDisabled();
  expect(screen.getByText('Dá para atualizar fora da fila, da seleção e da partida.')).toBeInTheDocument();
});

it('avisa quando já está na versão mais recente e deixa procurar de novo', async () => {
  const api = fakeApi(null);
  render(<Harness api={api} />);
  expect(await screen.findByText('Você está na versão mais recente.')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Procurar atualização' }));
  await vi.waitFor(() => expect(api.check).toHaveBeenCalledTimes(2));
});

it('avisa quando a consulta ou a instalação falham', async () => {
  const failing: AppUpdate = { ...fakeApi(null), check: () => Promise.reject(new Error('rede')) };
  const { unmount } = render(<Harness api={failing} />);
  expect(await screen.findByRole('alert')).toHaveTextContent('Não deu para procurar atualização agora.');
  unmount();

  render(<Harness api={fakeApi(info, () => Promise.reject(new Error('assinatura')))} />);
  fireEvent.click(await screen.findByRole('button', { name: 'Atualizar e reiniciar' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Não deu para instalar a atualização.');
});

it('procura de novo de tempos em tempos', async () => {
  vi.useFakeTimers();
  const api = fakeApi(null);
  renderHook(() => useAppUpdate(api));
  expect(api.check).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(CHECK_INTERVAL_MS); });
  expect(api.check).toHaveBeenCalledTimes(2);
});

it('some fora do app desktop', () => {
  const { container } = render(<Harness api={null} />);
  expect(container).toBeEmptyDOMElement();
});

it('só libera a instalação fora da fila, da seleção e da partida', () => {
  for (const phase of [null, 'None', 'Lobby', 'EndOfGame'] as const) expect(canInstallNow(phase)).toBe(true);
  for (const phase of ['Matchmaking', 'ReadyCheck', 'ChampSelect', 'GameStart', 'InProgress', 'Reconnect', 'PreEndOfGame'] as const) {
    expect(canInstallNow(phase)).toBe(false);
  }
});

import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import type { AgentState } from '../bindings/AgentState';
import type { CommandResult } from '../bindings/CommandResult';
import type { RunePage } from '../bindings/RunePage';
import type { Summoner } from '../bindings/Summoner';
import { testFullRuneCatalog, testRunePage } from '../test/runes';
import { makeState } from '../test/state';
import { RunePagesPanel } from './RunePagesPanel';

const summoner: Summoner = {
  puuid: 'p1', summonerId: 1, gameName: 'Victor', tagLine: 'BR1', displayName: '', summonerLevel: 30, profileIconId: 1,
};
const temporary: RunePage = {
  ...testRunePage, id: 20, name: 'Recomendada', isEditable: false, isDeletable: false, isTemporary: true,
};
function connection(overrides: Partial<AgentState> = {}): ConnectionSnapshot {
  return {
    status: 'connected', error: null,
    state: makeState({
      summoner,
      runePages: { pages: [testRunePage, temporary], currentPageId: 20, canCreatePage: true },
      runeCatalog: testFullRuneCatalog,
      ...overrides,
    }),
  };
}
function deferred() {
  let resolve!: (result: CommandResult) => void;
  const promise = new Promise<CommandResult>((res) => { resolve = res; });
  return { promise, resolve };
}
const panel = () => within(screen.getByRole('region', { name: 'Páginas de runas' }));
const page = (name: string) => within(panel().getByRole('listitem', { name }));
const editor = (name: string) => within(panel().getByRole('form', { name }));
const row = (form: ReturnType<typeof editor>, legend: string) => within(form.getByRole('group', { name: legend }));
const click = (element: HTMLElement) => act(() => { fireEvent.click(element); });

describe('RunePagesPanel', () => {
  it('lista as páginas e aplica uma só vez, mostrando o resultado depois da resposta', async () => {
    const request = deferred();
    const send = vi.fn(() => request.promise);
    render(<RunePagesPanel connection={connection()} send={send} />);
    expect(panel().getByRole('listitem', { name: 'Recomendada' })).toHaveAttribute('aria-current', 'true');
    expect(page('Recomendada').getByText(/Em uso · Temporária/)).toBeInTheDocument();
    expect(page('Mid').getByText(/Pressione o Ataque · Precisão · Dominação/)).toBeInTheDocument();
    expect(page('Recomendada').getByRole('button', { name: 'Usar Recomendada' })).toBeDisabled();
    expect(page('Recomendada').queryByRole('button', { name: /Editar|Apagar/ })).toBeNull();

    act(() => {
      fireEvent.click(page('Mid').getByRole('button', { name: 'Usar Mid' }));
      fireEvent.click(page('Mid').getByRole('button', { name: 'Usar Mid' }));
    });
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'selectRunePage', pageId: 10 });
    expect(panel().getByRole('status')).toHaveTextContent('Aplicando página');
    await act(async () => { request.resolve({ status: 'ok' }); });
    expect(panel().getByRole('status')).toHaveTextContent('Página aplicada.');
  });

  it('cria uma página nova com as escolhas do editor', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'ok' }));
    render(<RunePagesPanel connection={connection()} send={send} />);
    click(panel().getByRole('button', { name: 'Nova página' }));
    const form = editor('Nova página');
    const save = form.getByRole('button', { name: 'Criar e aplicar' });
    expect(form.getByRole('textbox', { name: 'Nome da página' })).toHaveValue('Página 3');
    expect(save).toBeDisabled();

    const primaryTree = row(form, 'Árvore primária');
    expect(primaryTree.getByRole('button', { name: 'Precisão', pressed: true })).toBeInTheDocument();
    click(primaryTree.getByRole('button', { name: 'Dominação' }));
    expect(primaryTree.getByRole('button', { name: 'Dominação', pressed: true })).toBeInTheDocument();
    click(row(form, 'Runa principal').getByRole('button', { name: 'Colheita Sombria' }));
    click(row(form, 'Linha 1').getByRole('button', { name: 'Golpe Desleal' }));
    click(row(form, 'Linha 2').getByRole('button', { name: 'Globos Oculares' }));
    click(row(form, 'Linha 3').getByRole('button', { name: 'Caçador Voraz' }));
    click(row(form, 'Árvore secundária').getByRole('button', { name: 'Feitiçaria' }));
    click(row(form, 'Secundária, linha 1').getByRole('button', { name: 'Manto Nimbus' }));
    click(row(form, 'Secundária, linha 3').getByRole('button', { name: 'Chamuscar' }));
    click(row(form, 'Ofensiva').getByRole('button', { name: 'Força Adaptativa' }));
    click(row(form, 'Flex').getByRole('button', { name: 'Força Adaptativa' }));
    expect(row(form, 'Ofensiva').getByRole('button', { name: 'Força Adaptativa' })).toHaveAttribute('aria-pressed', 'true');
    expect(form.getByText(/fragmento/)).toBeInTheDocument();
    click(row(form, 'Defesa').getByRole('button', { name: 'Vida Escalável' }));
    fireEvent.change(form.getByRole('textbox', { name: 'Nome da página' }), { target: { value: ' Jungle ' } });
    expect(save).toBeEnabled();

    await act(async () => { fireEvent.click(save); });
    expect(send).toHaveBeenCalledExactlyOnceWith({
      type: 'createRunePage',
      page: { name: 'Jungle', primaryStyleId: 8100, subStyleId: 8200, selectedPerkIds: [202, 211, 221, 231, 312, 331, 5008, 5008, 5001] },
    });
    expect(panel().getByRole('status')).toHaveTextContent('Página criada e aplicada.');
    expect(panel().queryByRole('form')).toBeNull();
  });

  it('edita uma página existente a partir das runas dela e mantém o editor no erro', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'lcu', message: 'página não editável' }));
    render(<RunePagesPanel connection={connection()} send={send} />);
    click(page('Mid').getByRole('button', { name: 'Editar Mid' }));
    const form = editor('Editar Mid');
    expect(row(form, 'Runa principal').getByRole('button', { name: 'Pressione o Ataque' })).toHaveAttribute('aria-pressed', 'true');
    expect(row(form, 'Secundária, linha 3').getByRole('button', { name: 'Caçador Voraz' })).toHaveAttribute('aria-pressed', 'true');
    click(row(form, 'Linha 1').getByRole('button', { name: 'Presença de Espírito' }));
    await act(async () => { fireEvent.click(form.getByRole('button', { name: 'Salvar página' })); });
    expect(send).toHaveBeenCalledExactlyOnceWith({
      type: 'updateRunePage', pageId: 10,
      page: { name: 'Mid', primaryStyleId: 8000, subStyleId: 8100, selectedPerkIds: [101, 112, 121, 131, 211, 231, 5008, 5010, 5011] },
    });
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível salvar a página: página não editável');
    expect(editor('Editar Mid').getByRole('button', { name: 'Presença de Espírito', pressed: true })).toBeInTheDocument();
    click(editor('Editar Mid').getByRole('button', { name: 'Cancelar' }));
    expect(panel().queryByRole('form')).toBeNull();
  });

  it('apaga só depois de confirmar', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'ok' }));
    render(<RunePagesPanel connection={connection()} send={send} />);
    click(page('Mid').getByRole('button', { name: 'Apagar Mid' }));
    click(page('Mid').getByRole('button', { name: 'Manter' }));
    expect(send).not.toHaveBeenCalled();
    click(page('Mid').getByRole('button', { name: 'Apagar Mid' }));
    await act(async () => { fireEvent.click(page('Mid').getByRole('button', { name: 'Confirmar exclusão' })); });
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'deleteRunePage', pageId: 10 });
    expect(panel().getByRole('status')).toHaveTextContent('Página apagada.');
  });

  it('não cria página sem espaço nem sem as árvores', () => {
    const send = vi.fn();
    const { rerender } = render(<RunePagesPanel connection={connection({
      runePages: { pages: [testRunePage], currentPageId: 10, canCreatePage: false },
    })} send={send} />);
    expect(panel().getByRole('button', { name: 'Nova página' })).toBeDisabled();
    expect(panel().getByText(/Apague uma para criar outra/)).toBeInTheDocument();
    rerender(<RunePagesPanel connection={connection({ runeCatalog: null })} send={send} />);
    expect(panel().getByRole('button', { name: 'Nova página' })).toBeDisabled();
    expect(page('Mid').getByRole('button', { name: 'Editar Mid' })).toBeDisabled();
    expect(panel().getByText(/Carregando as árvores/)).toBeInTheDocument();
  });

  it('fecha o editor quando a página some e avisa ao desconectar', () => {
    const send = vi.fn();
    const { rerender } = render(<RunePagesPanel connection={connection()} send={send} />);
    click(page('Mid').getByRole('button', { name: 'Editar Mid' }));
    rerender(<RunePagesPanel connection={connection({
      runePages: { pages: [temporary], currentPageId: 20, canCreatePage: true },
    })} send={send} />);
    expect(panel().queryByRole('form')).toBeNull();
    rerender(<RunePagesPanel connection={connection({ runePages: null })} send={send} />);
    expect(panel().getByText('Carregando páginas de runas…')).toBeInTheDocument();
    rerender(<RunePagesPanel connection={{ status: 'reconnecting', error: null, state: null }} send={send} />);
    expect(panel().getByText(/Aguarde a reconexão/)).toBeInTheDocument();
  });

  it('avisa quando o LoL desconecta no meio do comando', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'notConnected', message: '' }));
    render(<RunePagesPanel connection={connection()} send={send} />);
    await act(async () => { fireEvent.click(page('Mid').getByRole('button', { name: 'Usar Mid' })); });
    expect(panel().getByRole('alert')).toHaveTextContent('O League of Legends desconectou');
  });
});

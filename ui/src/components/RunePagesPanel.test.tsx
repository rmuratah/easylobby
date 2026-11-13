import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ConnectionSnapshot } from '../agent/client';
import type { AgentState } from '../bindings/AgentState';
import type { CommandResult } from '../bindings/CommandResult';
import type { RunePage } from '../bindings/RunePage';
import type { RunePages } from '../bindings/RunePages';
import type { Summoner } from '../bindings/Summoner';
import { testFullRuneCatalog, testRunePage } from '../test/runes';
import { makeState } from '../test/state';
import { RunePagesPanel } from './RunePagesPanel';
import { chooseOption } from '../test/select';

const summoner: Summoner = {
  puuid: 'p1', summonerId: 1, gameName: 'Victor', tagLine: 'BR1', displayName: '', summonerLevel: 30, profileIconId: 1,
};
const top: RunePage = { ...testRunePage, id: 11, name: 'Top', selectedPerkIds: [102, 111, 121, 131, 211, 231, 5008, 5010, 5011] };
const temporary: RunePage = {
  ...testRunePage, id: 20, name: 'Recomendada', isEditable: false, isDeletable: false, isTemporary: true,
};
const basePages: RunePages = { pages: [testRunePage, top, temporary], currentPageId: 10, canCreatePage: true };
function connection(overrides: Partial<AgentState> = {}): ConnectionSnapshot {
  return {
    status: 'connected', error: null,
    state: makeState({ summoner, runePages: basePages, runeCatalog: testFullRuneCatalog, ...overrides }),
  };
}
function deferred() {
  let resolve!: (result: CommandResult) => void;
  const promise = new Promise<CommandResult>((res) => { resolve = res; });
  return { promise, resolve };
}
const ok = () => vi.fn(async (): Promise<CommandResult> => ({ status: 'ok' }));
const panel = () => within(screen.getByRole('region', { name: 'Páginas de runas' }));
const editor = () => within(panel().getByRole('form', { name: 'Editar runas' }));
const row = (legend: string) => within(editor().getByRole('group', { name: legend }));
const pageSelect = () => editor().getByRole('combobox', { name: 'Página de runas' });
const click = (element: HTMLElement) => act(() => { fireEvent.click(element); });

describe('RunePagesPanel', () => {
  it('mostra a página em uso no editor e aplica outra pela escolha de página, uma vez só', async () => {
    const request = deferred();
    const send = vi.fn(() => request.promise);
    const { rerender } = render(<RunePagesPanel connection={connection()} send={send} />);
    expect(pageSelect()).toHaveAttribute('data-value', '10');
    expect(row('Runa principal').getByRole('button', { name: 'Pressione o Ataque' })).toHaveAttribute('aria-pressed', 'true');
    // Sem mudança, não há o que salvar.
    expect(editor().getByRole('button', { name: 'Salvar' })).toBeDisabled();

    chooseOption(pageSelect(), '11');
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'selectRunePage', pageId: 11 });
    expect(panel().getByRole('status')).toHaveTextContent('Aplicando página');
    expect(pageSelect()).toBeDisabled();
    // O editor só troca quando o agente confirma a página em uso.
    rerender(<RunePagesPanel connection={connection({ runePages: { ...basePages, currentPageId: 11 } })} send={send} />);
    await act(async () => { request.resolve({ status: 'ok' }); });
    expect(pageSelect()).toHaveAttribute('data-value', '11');
    expect(row('Runa principal').getByRole('button', { name: 'Ritmo Fatal' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('salva a página editada, com o nome trocado pelo lápis', async () => {
    const send = ok();
    render(<RunePagesPanel connection={connection()} send={send} />);
    click(editor().getByRole('button', { name: 'Renomear página' }));
    const name = editor().getByRole('textbox', { name: 'Nome da página' });
    expect(name).toHaveFocus();
    fireEvent.change(name, { target: { value: ' Mid AP ' } });
    // Enter fecha o nome sem salvar; a escolha de página volta com o nome novo.
    fireEvent.keyDown(name, { key: 'Enter' });
    expect(send).not.toHaveBeenCalled();
    expect(pageSelect()).toHaveTextContent(' Mid AP ', { normalizeWhitespace: false });
    click(row('Runa principal').getByRole('button', { name: 'Ritmo Fatal' }));
    await act(async () => { fireEvent.click(editor().getByRole('button', { name: 'Salvar' })); });
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'updateRunePage', pageId: 10, page: {
      name: 'Mid AP', primaryStyleId: 8000, subStyleId: 8100, selectedPerkIds: [102, 111, 121, 131, 211, 231, 5008, 5010, 5011],
    } });
    expect(panel().getByRole('status')).toHaveTextContent('Página salva.');
  });

  it('Esc no nome desfaz a troca de nome', () => {
    render(<RunePagesPanel connection={connection()} send={ok()} />);
    click(editor().getByRole('button', { name: 'Renomear página' }));
    const name = editor().getByRole('textbox', { name: 'Nome da página' });
    fireEvent.change(name, { target: { value: 'Outro' } });
    fireEvent.keyDown(name, { key: 'Escape' });
    expect(pageSelect()).toHaveTextContent(/^Mid$/);
    expect(editor().getByRole('button', { name: 'Salvar' })).toBeDisabled();
  });

  it('cria uma página pelo + com as runas do editor e um nome novo', async () => {
    const send = ok();
    const { rerender } = render(<RunePagesPanel connection={connection()} send={send} />);
    click(editor().getByRole('button', { name: 'Criar página' }));
    const name = editor().getByRole('textbox', { name: 'Nome da página' });
    expect(name).toHaveValue('Nova página de runas 3');
    fireEvent.change(name, { target: { value: 'Jungle' } });
    fireEvent.blur(name);
    expect(pageSelect()).toHaveTextContent(/^Jungle \(nova\)$/);
    expect(editor().getByRole('button', { name: 'Criar página' })).toBeDisabled();
    click(row('Runa principal').getByRole('button', { name: 'Ritmo Fatal' }));
    act(() => { fireEvent.click(editor().getByRole('button', { name: 'Salvar' })); });
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'createRunePage', page: {
      name: 'Jungle', primaryStyleId: 8000, subStyleId: 8100, selectedPerkIds: [102, 111, 121, 131, 211, 231, 5008, 5010, 5011],
    } });
    // O agente publica a página criada, já em uso, antes da resposta.
    const jungle = { ...top, id: 30, name: 'Jungle' };
    rerender(<RunePagesPanel connection={connection({ runePages: { ...basePages, pages: [...basePages.pages, jungle], currentPageId: 30 } })} send={send} />);
    await act(async () => {});
    expect(pageSelect()).toHaveAttribute('data-value', '30');
    expect(panel().getByRole('status')).toHaveTextContent('Página criada.');
  });

  it('descarta a página nova e volta para a página em uso', () => {
    render(<RunePagesPanel connection={connection()} send={ok()} />);
    click(editor().getByRole('button', { name: 'Criar página' }));
    click(editor().getByRole('button', { name: 'Descartar página nova' }));
    expect(pageSelect()).toHaveAttribute('data-value', '10');
  });

  it('exclui a página só depois de confirmar', async () => {
    const send = ok();
    render(<RunePagesPanel connection={connection()} send={send} />);
    click(editor().getByRole('button', { name: 'Excluir página' }));
    const confirm = within(editor().getByRole('alertdialog', { name: 'Excluir página' }));
    expect(confirm.getByText(/Excluir “Mid”\?/)).toBeInTheDocument();
    click(confirm.getByRole('button', { name: 'Manter' }));
    expect(send).not.toHaveBeenCalled();
    click(editor().getByRole('button', { name: 'Excluir página' }));
    await act(async () => {
      fireEvent.click(within(editor().getByRole('alertdialog')).getByRole('button', { name: 'Excluir' }));
    });
    expect(send).toHaveBeenCalledExactlyOnceWith({ type: 'deleteRunePage', pageId: 10 });
    expect(panel().getByRole('status')).toHaveTextContent('Página excluída.');
  });

  it('página recomendada não se edita nem se exclui, mas vira cópia pelo +', () => {
    render(<RunePagesPanel connection={connection({ runePages: { ...basePages, currentPageId: 20 } })} send={ok()} />);
    expect(pageSelect()).toHaveTextContent(/^Recomendada$/);
    expect(editor().getByRole('button', { name: 'Renomear página' })).toBeDisabled();
    expect(editor().getByRole('button', { name: 'Excluir página' })).toBeDisabled();
    expect(editor().getByRole('button', { name: 'Salvar' })).toBeDisabled();
    expect(row('Runa principal').getByRole('button', { name: 'Ritmo Fatal' })).toBeDisabled();
    expect(editor().getByText(/não pode ser editada/)).toBeInTheDocument();
    click(editor().getByRole('button', { name: 'Criar página' }));
    expect(row('Runa principal').getByRole('button', { name: 'Pressione o Ataque' })).toHaveAttribute('aria-pressed', 'true');
    expect(editor().getByRole('button', { name: 'Salvar' })).toBeEnabled();
  });

  it('sem espaço para página, o + fica travado e diz por quê', () => {
    render(<RunePagesPanel connection={connection({ runePages: { ...basePages, canCreatePage: false } })} send={ok()} />);
    const create = editor().getByRole('button', { name: 'Criar página' });
    expect(create).toBeDisabled();
    expect(create).toHaveAttribute('title', expect.stringMatching(/Todas as páginas estão ocupadas/));
  });

  it('mostra o nome e a descrição completa da runa ao passar o mouse', () => {
    render(<RunePagesPanel connection={connection()} send={ok()} />);
    const rune = row('Runa principal').getByRole('button', { name: 'Ritmo Fatal' });
    expect(screen.queryByRole('tooltip')).toBeNull();
    fireEvent.mouseEnter(rune);
    const tooltip = screen.getByRole('tooltip');
    expect(tooltip).toHaveTextContent('Ritmo Fatal');
    expect(tooltip).toHaveTextContent(/Descrição completa de Ritmo Fatal\.\s+Tempo de Recarga: 10s/);
    expect(rune).toHaveAttribute('aria-describedby', tooltip.id);
    fireEvent.mouseLeave(rune);
    expect(screen.queryByRole('tooltip')).toBeNull();
    // Pelo teclado também.
    fireEvent.focus(rune);
    expect(screen.getByRole('tooltip')).toHaveTextContent('Ritmo Fatal');
  });

  it('avisa quando o LoL desconecta, e quando falta o catálogo', () => {
    const { rerender } = render(<RunePagesPanel connection={connection({ runeCatalog: null })} send={ok()} />);
    expect(panel().getByText('Carregando as árvores de runas…')).toBeInTheDocument();
    rerender(<RunePagesPanel connection={{ ...connection(), status: 'reconnecting' }} send={ok()} />);
    expect(panel().getByText(/Aguarde a reconexão/)).toBeInTheDocument();
    expect(panel().queryByRole('form')).toBeNull();
  });

  it('mostra o erro do agente e mantém o rascunho', async () => {
    const send = vi.fn(async (): Promise<CommandResult> => ({ status: 'error', code: 'lcu', message: 'página não editável' }));
    render(<RunePagesPanel connection={connection()} send={send} />);
    click(row('Runa principal').getByRole('button', { name: 'Ritmo Fatal' }));
    await act(async () => { fireEvent.click(editor().getByRole('button', { name: 'Salvar' })); });
    expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível salvar a página: página não editável');
    expect(row('Runa principal').getByRole('button', { name: 'Ritmo Fatal' })).toHaveAttribute('aria-pressed', 'true');
  });
});

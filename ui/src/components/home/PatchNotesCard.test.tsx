import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { PatchNotesCard } from './PatchNotesCard';
import type { PatchNotes } from '../../bindings/PatchNotes';

const notes: PatchNotes = {
  version: '26.22',
  title: 'Notas da Atualização 26.22 do League of Legends',
  summary: 'K/DA chegando pra botar fogo no parquinho.',
  url: 'https://www.leagueoflegends.com/pt-br/news/game-updates/league-of-legends-patch-26-22-notes/',
};

describe('PatchNotesCard', () => {
  it('mostra a versão e o resumo e abre as notas pelo agente', async () => {
    const send = vi.fn().mockResolvedValue({ status: 'ok' });
    render(<PatchNotesCard notes={notes} send={send} />);
    const card = screen.getByRole('article', { name: 'Notas da atualização' });
    expect(card).toHaveTextContent('26.22');
    expect(screen.getByRole('heading', { name: 'Atualização 26.22' })).toBeInTheDocument();
    expect(card).toHaveTextContent('K/DA chegando');
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    expect(send).toHaveBeenCalledWith({ type: 'openPatchNotes' });
    expect(await screen.findByRole('button', { name: 'Ler as notas' })).toBeEnabled();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
  it('sem resumo, convida a ler no site', () => {
    render(<PatchNotesCard notes={{ ...notes, title: null, summary: null }} send={vi.fn()} />);
    expect(screen.getByRole('article', { name: 'Notas da atualização' })).toHaveTextContent('site oficial');
  });
  it('avisa quando o navegador não abre', async () => {
    const send = vi.fn().mockResolvedValue({ status: 'error', code: 'launchFailed', message: 'x' });
    render(<PatchNotesCard notes={notes} send={send} />);
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Não foi possível abrir');
  });
  it('sem notas, não mostra o botão', () => {
    render(<PatchNotesCard notes={null} send={vi.fn()} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });
});

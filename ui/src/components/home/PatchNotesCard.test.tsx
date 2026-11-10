import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { PatchNotesCard } from './PatchNotesCard';
import type { PatchNotes } from '../../bindings/PatchNotes';

const notes: PatchNotes = {
  version: '26.22',
  title: 'Notas da Atualização 26.22 do League of Legends',
  summary: 'K/DA chegando pra botar fogo no parquinho.',
  url: 'https://www.leagueoflegends.com/pt-br/news/game-updates/league-of-legends-patch-26-22-notes/',
  sections: [
    {
      title: 'Campeões',
      changes: [{
        title: 'Corki',
        lines: [
          { kind: 'text', text: 'Corki está forte demais.' },
          { kind: 'heading', text: 'Q - Bomba de Fósforo' },
          { kind: 'bullet', text: 'Dano: 90 ⇒ 80' },
          { kind: 'bullet', text: 'Recarga menor' },
        ],
      }],
    },
    { title: 'Correções de bugs', changes: [{ title: null, lines: [{ kind: 'bullet', text: 'Mel não trava mais.' }] }] },
  ],
};

describe('PatchNotesCard', () => {
  it('mostra a versão e o resumo', () => {
    render(<PatchNotesCard notes={notes} send={vi.fn()} />);
    const card = screen.getByRole('article', { name: 'Notas da atualização' });
    expect(card).toHaveTextContent('26.22');
    expect(screen.getByRole('heading', { name: 'Atualização 26.22' })).toBeInTheDocument();
    expect(card).toHaveTextContent('K/DA chegando');
  });
  it('"Ler as notas" abre o card com as mudanças, sem abrir o navegador', () => {
    const send = vi.fn();
    render(<PatchNotesCard notes={notes} send={send} />);
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    const dialog = screen.getByRole('dialog', { name: notes.title! });
    expect(send).not.toHaveBeenCalled();
    const champions = within(dialog).getByRole('region', { name: 'Campeões' });
    expect(within(champions).getByRole('heading', { name: 'Corki' })).toBeInTheDocument();
    expect(within(champions).getAllByRole('listitem').map(li => li.textContent)).toEqual(['Dano: 90 ⇒ 80', 'Recarga menor']);
    expect(champions).toHaveTextContent('Q - Bomba de Fósforo');
    expect(within(dialog).getByRole('region', { name: 'Correções de bugs' })).toHaveTextContent('Mel não trava mais.');
  });
  it('fecha pelo botão e pelo Esc', () => {
    render(<PatchNotesCard notes={notes} send={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    fireEvent.click(within(screen.getByRole('dialog')).getAllByRole('button', { name: 'Fechar' })[0]);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
  it('"Abrir no site" pede ao agente para abrir o navegador', async () => {
    const send = vi.fn().mockResolvedValue({ status: 'ok' });
    render(<PatchNotesCard notes={notes} send={send} />);
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    fireEvent.click(screen.getByRole('button', { name: 'Abrir no site' }));
    expect(send).toHaveBeenCalledWith({ type: 'openPatchNotes' });
    expect(await screen.findByRole('button', { name: 'Abrir no site' })).toBeEnabled();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
  it('avisa quando o navegador não abre', async () => {
    const send = vi.fn().mockResolvedValue({ status: 'error', code: 'launchFailed', message: 'x' });
    render(<PatchNotesCard notes={notes} send={send} />);
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    fireEvent.click(screen.getByRole('button', { name: 'Abrir no site' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Não foi possível abrir');
  });
  it('sem as mudanças, o card manda para o site', () => {
    render(<PatchNotesCard notes={{ ...notes, title: null, summary: null, sections: [] }} send={vi.fn()} />);
    expect(screen.getByRole('article', { name: 'Notas da atualização' })).toHaveTextContent('Veja o que mudou');
    fireEvent.click(screen.getByRole('button', { name: 'Ler as notas' }));
    const dialog = screen.getByRole('dialog', { name: 'Atualização 26.22' });
    expect(dialog).toHaveTextContent('site oficial');
    expect(within(dialog).getByRole('button', { name: 'Abrir no site' })).toBeInTheDocument();
  });
  it('sem notas, não mostra o botão', () => {
    render(<PatchNotesCard notes={null} send={vi.fn()} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });
});

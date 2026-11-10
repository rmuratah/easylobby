import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { Lobby } from '../bindings/Lobby';
import type { PlayerSlot } from '../bindings/PlayerSlot';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testLobby, testState } from '../test/lobby';
import { testFullRuneCatalog } from '../test/runes';

class Socket {
  static OPEN = 1;
  static instances: Socket[] = [];
  readyState = 1;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  send = vi.fn();
  close = vi.fn();
  constructor(readonly url: string) { Socket.instances.push(this); }
  receive(message: ServerMessage) { this.onmessage?.({ data: JSON.stringify(message) }); }
}
const socket = () => Socket.instances.at(-1)!;
const panel = () => within(screen.getByRole('region', { name: 'Lobby' }));
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
const dialog = () => within(screen.getByRole('dialog', { name: 'Escolha seus campeões' }));
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
async function ok() {
  const id = request().id;
  await act(async () => socket().receive({ type: 'response', id, result: { status: 'ok' } }));
}
beforeEach(() => { configure({ defaultHidden: true }); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

const slot = (championId: number, position: PlayerSlot['position'], spells: [number, number] = [4, 14]): PlayerSlot => ({
  championId, skinId: championId * 1000, position, spell1Id: spells[0], spell2Id: spells[1],
  runeStyleId: 8000, runeSubStyleId: 8300, keystoneId: 8010, perkIds: [8010, 9111, 9104, 8299, 8345, 8347, 5005, 5008, 5001],
});
const swiftplay = (playerSlots: PlayerSlot[] = []): Lobby => ({
  ...testLobby, queueId: 480, queue: 'swiftplay', maxLobbySize: 5, showPositionSelector: false, showQuickPlaySlots: true,
  scarcePositions: ['jungle', 'bottom'], members: [{ ...testLobby.members[0], playerSlots }],
});
const champions = { champions: [
  { id: 266, name: 'Aatrox', alias: 'Aatrox', roles: ['fighter'], positions: [] },
  { id: 22, name: 'Ashe', alias: 'Ashe', roles: ['marksman'], positions: [] },
  { id: 1, name: 'Annie', alias: 'Annie', roles: ['mage'], positions: [] },
] };
const spells = { spells: [
  { id: 4, name: 'Flash', description: '', summonerLevel: 7, gameModes: ['SWIFTPLAY'], queues: ['swiftplay' as const] },
  { id: 14, name: 'Incendiar', description: '', summonerLevel: 9, gameModes: ['SWIFTPLAY'], queues: ['swiftplay' as const] },
  { id: 12, name: 'Teleporte', description: '', summonerLevel: 7, gameModes: ['SWIFTPLAY'], queues: ['swiftplay' as const] },
] };
function open(name: RegExp) {
  fireEvent.click(panel().getByRole('button', { name }));
}

describe('posições e campeões do Jogo Dinâmico (#218)', () => {
  it('escolhe as duas posições e os dois campeões no card e salva quando completa', async () => {
    render(<App />);
    hello({ lobby: swiftplay(), gameflowPhase: 'Lobby', championCatalog: champions, playableChampions: [22, 266] });
    expect(panel().getByText(/escolha as duas posições e um campeão para cada uma no seu card/)).toBeInTheDocument();

    open(/^Primeira posição: Não selecionado/);
    // Sem abas de posição e feitiços: a posição abre embaixo do slot.
    expect(dialog().queryByRole('tab', { name: 'Posição' })).not.toBeInTheDocument();
    expect(dialog().queryByRole('tab', { name: 'Feitiços' })).not.toBeInTheDocument();
    const menu = () => within(dialog().getByRole('group', { name: 'Escolher a posição' }));
    // Runas e skin só depois dos dois slots salvos.
    expect(dialog().getByRole('tab', { name: 'Skins' })).toBeDisabled();
    expect(menu().getByRole('button', { name: 'Selva (prioritária)' })).toBeInTheDocument();
    fireEvent.click(menu().getByRole('button', { name: 'Atirador (prioritária)' }));
    // Depois da posição, o pop-up passa sozinho para o campeão do mesmo slot.
    expect(dialog().getByRole('tab', { name: 'Campeão' })).toHaveAttribute('aria-selected', 'true');
    // Só os campeões que o jogador pode jogar.
    expect(dialog().queryByRole('button', { name: 'Annie' })).not.toBeInTheDocument();
    fireEvent.change(dialog().getByRole('searchbox', { name: 'Buscar campeão' }), { target: { value: 'as' } });
    fireEvent.click(dialog().getByRole('button', { name: 'Ashe' }));
    // E depois para o que falta no outro slot.
    expect(dialog().getByRole('region', { name: 'Escolha da segunda posição' })).toBeInTheDocument();
    fireEvent.click(menu().getByRole('button', { name: 'Topo' }));
    // Ainda falta o campeão da segunda posição: nada sai.
    expect(socket().send).not.toHaveBeenCalled();
    fireEvent.click(dialog().getByRole('button', { name: 'Aatrox' }));

    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
      { championId: 266, position: 'top', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
    ] });
    expect(panel().getByRole('status')).toHaveTextContent('Salvando posições e campeões');
    act(() => socket().receive({ type: 'lobby', lobby: swiftplay([slot(22, 'bottom'), slot(266, 'top')]) }));
    await ok();
    expect(panel().getByRole('button', { name: 'Primeira posição: Ashe' })).toBeInTheDocument();
    expect(panel().getByRole('button', { name: 'Segunda posição: Topo' })).toBeInTheDocument();
  });

  it('troca a posição entre os slots e muda os feitiços de um slot salvo', async () => {
    render(<App />);
    hello({ lobby: swiftplay([slot(22, 'bottom'), slot(266, 'top')]), gameflowPhase: 'Lobby', championCatalog: champions,
      summonerSpellCatalog: spells });
    open(/^Segunda posição: Topo/);
    fireEvent.click(dialog().getByRole('button', { name: 'Atirador (prioritária)' }));
    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'top', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
      { championId: 266, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
    ] });
    await ok();

    open(/^Segunda posição, feitiço F: Incendiar/);
    const spellMenu = within(dialog().getByRole('group', { name: 'Feitiço da tecla F' }));
    fireEvent.click(spellMenu.getByRole('button', { name: 'Teleporte' }));
    expect(dialog().queryByRole('group', { name: 'Feitiço da tecla F' })).not.toBeInTheDocument();
    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
      { championId: 266, position: 'top', spell1Id: 4, spell2Id: 12, runePageId: null, runes: null, skinId: null },
    ] });
  });

  it('edita as runas do slot no pop-up e salva (#331)', () => {
    render(<App />);
    const runed = { ...slot(266, 'top'), runeStyleId: 8000, runeSubStyleId: 8100, keystoneId: 101,
      perkIds: [101, 111, 121, 131, 211, 231, 5008, 5010, 5011] };
    hello({ lobby: swiftplay([slot(22, 'bottom'), runed]), gameflowPhase: 'Lobby', championCatalog: champions,
      runeCatalog: testFullRuneCatalog });
    open(/^Segunda posição: runas/);
    const editor = within(dialog().getByRole('form', { name: 'Editar runas' }));
    // Começa com as runas do slot, e o nome aparece no hover.
    expect(editor.getByRole('button', { name: 'Pressione o Ataque' })).toHaveAttribute('aria-pressed', 'true');
    expect(editor.getByRole('button', { name: 'Ritmo Fatal' })).toHaveAttribute('title', 'Ritmo Fatal');
    fireEvent.click(editor.getByRole('button', { name: 'Ritmo Fatal' }));
    // Trocar a árvore primária esvazia as linhas dela: o Salvar trava até completar.
    const primaryTree = within(editor.getByRole('group', { name: 'Árvore primária' }));
    fireEvent.click(primaryTree.getByRole('button', { name: 'Dominação' }));
    expect(editor.getByRole('button', { name: 'Salvar' })).toBeDisabled();
    fireEvent.click(primaryTree.getByRole('button', { name: 'Precisão' }));
    // A secundária volta vazia: escolhe as duas de novo.
    for (const name of ['Ritmo Fatal', 'Triunfo', 'Lenda: Espontaneidade', 'Golpe de Misericórdia', 'Golpe Desleal', 'Caçador Voraz']) {
      fireEvent.click(editor.getByRole('button', { name }));
    }
    fireEvent.click(editor.getByRole('button', { name: 'Salvar' }));
    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
      { championId: 266, position: 'top', spell1Id: null, spell2Id: null, runePageId: null,
        runes: { perkIds: [102, 111, 121, 131, 211, 231, 5008, 5010, 5011], styleId: 8000, subStyleId: 8100 }, skinId: null },
    ] });
  });

  it('escolhe a skin do slot entre as da conta (#403)', async () => {
    render(<App />);
    hello({ lobby: swiftplay([slot(22, 'bottom'), slot(266, 'top')]), gameflowPhase: 'Lobby', championCatalog: champions });
    open(/^Primeira posição: Ashe/);
    fireEvent.click(dialog().getByRole('tab', { name: 'Skins' }));
    expect(request().command).toEqual({ type: 'openChampionSkins', championId: 22 });
    expect(dialog().getByText('Carregando as skins…')).toBeInTheDocument();
    const skin = (id: number, name: string, owned: boolean, isBase = false) => ({ id, name, isBase, owned, rarity: null, chromas: [] });
    act(() => socket().receive({ type: 'championSkins', championSkins: { championId: 22, skins: [
      skin(22000, 'Ashe', false, true), skin(22001, 'Ashe Freljord', true), skin(22002, 'Ashe Sherwood', false),
    ] } }));
    await ok();
    // A padrão sempre aparece; as que a conta não tem, não.
    expect(dialog().getByRole('button', { name: 'Ashe (padrão)' })).toHaveAttribute('aria-pressed', 'true');
    expect(dialog().queryByRole('button', { name: 'Ashe Sherwood' })).not.toBeInTheDocument();
    fireEvent.click(dialog().getByRole('button', { name: 'Ashe Freljord' }));
    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: 22001 },
      { championId: 266, position: 'top', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
    ] });
  });

  it('filtra a grade do slot pela rota, começando na posição do slot (#404)', async () => {
    render(<App />);
    const laned = { champions: [
      { id: 266, name: 'Aatrox', alias: 'Aatrox', roles: ['fighter'], positions: ['top' as const] },
      { id: 22, name: 'Ashe', alias: 'Ashe', roles: ['marksman'], positions: ['bottom' as const, 'utility' as const] },
      { id: 1, name: 'Annie', alias: 'Annie', roles: ['mage'], positions: ['middle' as const] },
    ] };
    hello({ lobby: swiftplay([slot(22, 'bottom'), slot(266, 'top')]), gameflowPhase: 'Lobby', championCatalog: laned });
    open(/^Segunda posição: Aatrox/);
    const lanes = within(dialog().getByRole('list', { name: 'Filtrar por rota' }));
    expect(lanes.getByRole('button', { name: 'Topo' })).toHaveAttribute('aria-pressed', 'true');
    expect(dialog().getByRole('button', { name: 'Aatrox' })).toBeInTheDocument();
    expect(dialog().queryByRole('button', { name: 'Annie' })).not.toBeInTheDocument();
    // Clicar na rota ativa tira o filtro.
    fireEvent.click(lanes.getByRole('button', { name: 'Topo' }));
    expect(dialog().getByRole('button', { name: 'Annie' })).toBeInTheDocument();
    // Enter escolhe o primeiro da busca.
    const search = dialog().getByRole('searchbox', { name: 'Buscar campeão' });
    // Esc com texto limpa a busca sem fechar o pop-up.
    fireEvent.change(search, { target: { value: 'xyz' } });
    fireEvent.keyDown(search, { key: 'Escape' });
    expect(search).toHaveValue('');
    expect(screen.getByRole('dialog', { name: 'Escolha seus campeões' })).toBeInTheDocument();
    fireEvent.change(search, { target: { value: 'ann' } });
    fireEvent.keyDown(search, { key: 'Enter' });
    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
      { championId: 1, position: 'top', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
    ] });
  });

  it('página salva só carrega no editor e nada sai sem o Salvar (#417)', () => {
    render(<App />);
    const runed = { ...slot(266, 'top'), runeStyleId: 8000, runeSubStyleId: 8100, keystoneId: 101,
      perkIds: [101, 111, 121, 131, 211, 231, 5008, 5010, 5011] };
    const page = { id: 7, name: 'Blitz', primaryStyleId: 8000, subStyleId: 8100, selectedPerkIds: [102, 111, 121, 131, 211, 231, 5008, 5010, 5011],
      isEditable: true, isDeletable: true, isTemporary: false, isValid: true };
    hello({ lobby: swiftplay([slot(22, 'bottom'), runed]), gameflowPhase: 'Lobby', championCatalog: champions,
      runeCatalog: testFullRuneCatalog, runePages: { pages: [page], currentPageId: 7, canCreatePage: true } });
    open(/^Segunda posição: runas/);
    const editor = within(dialog().getByRole('form', { name: 'Editar runas' }));
    expect(editor.getByRole('button', { name: 'Pressione o Ataque' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.change(editor.getByRole('combobox', { name: /Carregar página salva/ }), { target: { value: '7' } });
    // A página entra no editor, mas nada vai para o LoL.
    expect(editor.getByRole('button', { name: 'Ritmo Fatal' })).toHaveAttribute('aria-pressed', 'true');
    expect(socket().send).not.toHaveBeenCalled();
    fireEvent.click(editor.getByRole('button', { name: 'Salvar' }));
    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
      { championId: 266, position: 'top', spell1Id: null, spell2Id: null, runePageId: null,
        runes: { perkIds: [102, 111, 121, 131, 211, 231, 5008, 5010, 5011], styleId: 8000, subStyleId: 8100 }, skinId: null },
    ] });
  });

  it('inverte os dois slots pelo botão do pop-up (#405)', async () => {
    render(<App />);
    hello({ lobby: swiftplay([slot(22, 'bottom'), slot(266, 'top')]), gameflowPhase: 'Lobby', championCatalog: champions });
    open(/^Primeira posição: Ashe/);
    fireEvent.click(dialog().getByRole('button', { name: 'Inverter os slots' }));
    expect(request().command).toEqual({ type: 'setPlayerSlots', slots: [
      { championId: 266, position: 'top', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
      { championId: 22, position: 'bottom', spell1Id: null, spell2Id: null, runePageId: null, runes: null, skinId: null },
    ] });
    await ok();
    // Esc fecha o pop-up.
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Escolha seus campeões' })).not.toBeInTheDocument();
  });

  it('avisa sem posição prioritária e não deixa mexer na fila', () => {
    render(<App />);
    hello({ lobby: swiftplay([slot(1, 'middle'), slot(266, 'top')]), gameflowPhase: 'Lobby', championCatalog: champions });
    expect(panel().getByText(/Escolha ao menos uma posição prioritária/)).toBeInTheDocument();
    act(() => socket().receive({ type: 'gameflowPhase', phase: 'Matchmaking' }));
    expect(panel().queryByRole('button', { name: /^Primeira posição/ })).not.toBeInTheDocument();
    expect(panel().getByRole('img', { name: 'Primeira posição: Annie' })).toBeInTheDocument();
  });
});

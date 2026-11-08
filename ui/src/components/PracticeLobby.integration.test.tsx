import { configure, act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../App';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { CustomLobby } from '../bindings/CustomLobby';
import type { CustomMember } from '../bindings/CustomMember';
import type { Lobby } from '../bindings/Lobby';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testLobby, testState } from '../test/lobby';

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
const team = (name: string) => within(panel().getByRole('region', { name }));
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, gameflowPhase: 'Lobby', ...overrides } }));
}
async function ok() {
  const id = request().id;
  await act(async () => socket().receive({ type: 'response', id, result: { status: 'ok' } }));
}
beforeEach(() => { configure({ defaultHidden: true }); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { configure({ defaultHidden: false }); vi.unstubAllGlobals(); });

const me: CustomMember = { puuid: 'local', summonerId: 1, summonerIconId: 1, isLeader: true, bot: null };
const ashe: CustomMember = { puuid: '', summonerId: 0, summonerIconId: 0, isLeader: false,
  bot: { uuid: 'bot-ashe', championId: 22, position: 'bottom', difficulty: 'beginner' } };
const practice = (custom: Partial<CustomLobby> = {}, leader = true): Lobby => ({
  ...testLobby, queueId: 3140, queue: null, maxLobbySize: 10, showPositionSelector: false, isLeader: leader,
  custom: { settings: { name: 'Modo de Treino', gameMode: 'PRACTICETOOL', mapId: 11, map: 'summonersRift', pickType: 'blindPick', spectators: 'none' }, teamSize: 5, team1: [{ ...me, isLeader: leader }], team2: [], spectators: [], maxSpectators: 4, ...custom },
});
const champions = { champions: [
  { id: 266, name: 'Aatrox', alias: 'Aatrox', roles: ['fighter'], positions: [] },
  { id: 22, name: 'Ashe', alias: 'Ashe', roles: ['marksman'], positions: [] },
  { id: 1, name: 'Annie', alias: 'Annie', roles: ['mage'], positions: [] },
] };

describe('lobby do Modo de Treino (#229)', () => {
  it('mostra as duas equipes com 5 vagas e os bots', () => {
    render(<App />);
    hello({ lobby: practice({ team2: [ashe] }), championCatalog: champions });
    expect(panel().getByText("Summoner's Rift · 5v5 · Escolha às cegas · Modo de Treino")).toBeInTheDocument();
    expect(team('Equipe 1').getAllByRole('listitem', { name: 'Vaga livre' })).toHaveLength(4);
    expect(team('Equipe 2').getAllByRole('listitem', { name: 'Vaga livre' })).toHaveLength(4);
    expect(team('Equipe 2').getByText('Ashe')).toBeInTheDocument();
    expect(team('Equipe 2').getByRole('combobox', { name: 'Dificuldade do bot Ashe' })).toHaveValue('beginner');
    const position = team('Equipe 2').getByRole('combobox', { name: 'Posição do bot Ashe' });
    expect(position).toHaveValue('bottom');
    // Só as 5 rotas.
    expect(within(position).getAllByRole('option').map(o => o.textContent)).toEqual(['Topo', 'Selva', 'Meio', 'Atirador', 'Suporte']);
    // Juntar-se só na equipe do outro lado.
    expect(team('Equipe 2').getByRole('button', { name: 'Juntar-se' })).toBeInTheDocument();
    expect(team('Equipe 1').queryByRole('button', { name: 'Juntar-se' })).not.toBeInTheDocument();
    // A busca de partida não vale na personalizada.
    const busca = within(screen.getByRole('region', { name: 'Busca de partida' }));
    expect(busca.getByRole('button', { name: 'Iniciar busca' })).toBeDisabled();
  });

  it('adiciona um bot pelo pop-up com busca, muda e tira', async () => {
    render(<App />);
    hello({ lobby: practice({ team2: [ashe] }), championCatalog: champions, availableBots: [1, 22] });
    fireEvent.click(team('Equipe 2').getAllByRole('button', { name: /Adicionar bot/ })[0]);
    const dialog = within(screen.getByRole('dialog', { name: 'Novo bot na Equipe 2' }));
    // Só os campeões que podem ser bots.
    expect(dialog.queryByRole('button', { name: 'Aatrox' })).not.toBeInTheDocument();
    fireEvent.change(dialog.getByRole('searchbox', { name: 'Buscar campeão' }), { target: { value: 'ann' } });
    expect(dialog.queryByRole('button', { name: 'Ashe' })).not.toBeInTheDocument();
    fireEvent.click(dialog.getByRole('button', { name: 'Annie' }));
    // Ashe já é atirador: o bot novo fica na primeira rota livre.
    expect(request().command).toEqual({ type: 'addBot', team: 'team2', championId: 1, difficulty: 'intro', position: 'top' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    await ok();

    fireEvent.change(team('Equipe 2').getByRole('combobox', { name: 'Dificuldade do bot Ashe' }), { target: { value: 'intermediate' } });
    expect(request().command).toEqual({ type: 'updateBot', botUuid: 'bot-ashe', championId: 22, difficulty: 'intermediate', position: 'bottom' });
    await ok();
    fireEvent.click(team('Equipe 2').getByRole('button', { name: /Trocar o campeão do bot/ }));
    fireEvent.click(within(screen.getByRole('dialog', { name: 'Campeão do bot' })).getByRole('button', { name: 'Annie' }));
    expect(request().command).toEqual({ type: 'updateBot', botUuid: 'bot-ashe', championId: 1, difficulty: 'beginner', position: 'bottom' });
    await ok();
    fireEvent.click(team('Equipe 2').getByRole('button', { name: 'Tirar o bot Ashe' }));
    expect(request().command).toEqual({ type: 'removeBot', botUuid: 'bot-ashe' });
    await ok();

    fireEvent.click(team('Equipe 2').getByRole('button', { name: 'Juntar-se' }));
    expect(request().command).toEqual({ type: 'switchTeam', team: 'team2' });
    await ok();
    fireEvent.click(panel().getByRole('button', { name: 'Iniciar partida' }));
    expect(request().command).toEqual({ type: 'startCustomGame' });
  });

  it('quem não é o líder só vê os bots e pode trocar de equipe', () => {
    render(<App />);
    hello({ lobby: practice({ team2: [ashe] }, false), championCatalog: champions });
    expect(panel().queryByRole('button', { name: /Adicionar bot/ })).not.toBeInTheDocument();
    expect(panel().queryByRole('button', { name: 'Iniciar partida' })).not.toBeInTheDocument();
    expect(panel().queryByRole('button', { name: 'Tirar o bot Ashe' })).not.toBeInTheDocument();
    expect(team('Equipe 2').getByRole('combobox', { name: 'Dificuldade do bot Ashe' })).toBeDisabled();
    expect(team('Equipe 2').getByRole('button', { name: 'Juntar-se' })).toBeEnabled();
  });
});

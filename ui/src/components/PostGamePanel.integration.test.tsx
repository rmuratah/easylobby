import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from '../App';
import { HONOR_CONFIRM_MS } from './HonorVote';
import { STATS_TIMEOUT_MS } from './PostGamePanel';
import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { HonorBallot } from '../bindings/HonorBallot';
import type { ServerMessage } from '../bindings/ServerMessage';
import { testGameResult } from '../test/gameResult';
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
const panel = () => within(screen.getByRole('region', { name: 'Fim de partida' }));
const request = (): ClientMessage => JSON.parse(socket().send.mock.lastCall![0]);
function hello(overrides: Partial<AgentState> = {}) {
  act(() => socket().receive({ type: 'hello', protocol: 1, agentVersion: '0.1.0', state: { ...testState, ...overrides } }));
}
async function respond(result: Extract<ServerMessage, { type: 'response' }>['result'] = { status: 'ok' }) {
  await act(async () => socket().receive({ type: 'response', id: request().id, result }));
}

beforeEach(() => { vi.useFakeTimers(); Socket.instances = []; vi.stubGlobal('WebSocket', Socket); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

it('joga de novo pelo playAgain e troca para o lobby quando a fase muda', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame' });
  fireEvent.click(panel().getByRole('button', { name: 'Jogar de novo' }));
  expect(request().command).toEqual({ type: 'playAgain' });
  expect(panel().getByRole('button', { name: 'Voltando ao lobby…' })).toBeDisabled();
  await respond();
  act(() => socket().receive({ type: 'lobby', lobby: testLobby }));
  act(() => socket().receive({ type: 'gameflowPhase', phase: 'Lobby' }));
  expect(screen.queryByRole('region', { name: 'Fim de partida' })).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: 'Sair do lobby' })).toBeEnabled();
});

it('mostra o erro da LCU e deixa tentar de novo', async () => {
  render(<App />);
  hello({ gameflowPhase: 'PreEndOfGame' });
  fireEvent.click(panel().getByRole('button', { name: 'Jogar de novo' }));
  await respond({ status: 'error', code: 'lcu', message: 'fora do pós-jogo' });
  expect(panel().getByRole('alert')).toHaveTextContent('Não foi possível jogar de novo: fora do pós-jogo');
  expect(panel().getByRole('button', { name: 'Jogar de novo' })).toBeEnabled();
});

it('avisa que o resultado está sendo calculado em WaitingForStats', () => {
  render(<App />);
  hello({ gameflowPhase: 'WaitingForStats' });
  expect(panel().getByText('Calculando o resultado…')).toBeInTheDocument();
  expect(panel().getByRole('button', { name: 'Jogar de novo' })).toBeEnabled();
});

it('sai do fim de partida e volta ao início', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  fireEvent.click(panel().getByRole('button', { name: 'Sair' }));
  expect(request().command).toEqual({ type: 'leavePostGame' });
  expect(panel().getByRole('button', { name: 'Saindo…' })).toBeDisabled();
  expect(panel().getByRole('button', { name: 'Jogar de novo' })).toBeDisabled();
  await respond();
  act(() => socket().receive({ type: 'lobby', lobby: null }));
  act(() => socket().receive({ type: 'gameflowPhase', phase: 'None' }));
  expect(screen.queryByRole('region', { name: 'Fim de partida' })).not.toBeInTheDocument();
});

it('Enter joga de novo, mas não quando o foco está num botão', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', gameResult: testGameResult });
  expect(panel().getByRole('button', { name: 'Jogar de novo' })).toHaveAttribute('aria-keyshortcuts', 'Enter');
  expect(panel().getByText('Enter joga de novo')).toBeInTheDocument();
  fireEvent.keyDown(panel().getByRole('button', { name: 'Sair' }), { key: 'Enter' });
  expect(socket().send).not.toHaveBeenCalled();
  fireEvent.keyDown(document.body, { key: 'Enter' });
  expect(request().command).toEqual({ type: 'playAgain' });
  // Um segundo Enter enquanto o primeiro está em andamento não manda de novo.
  fireEvent.keyDown(document.body, { key: 'Enter' });
  expect(socket().send).toHaveBeenCalledTimes(1);
  await respond();
});

it('Enter não joga de novo durante a votação de honra', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.keyDown(document.body, { key: 'Enter' });
  expect(socket().send).not.toHaveBeenCalled();
});

it('avisa quando as estatísticas não chegam e deixa sair', () => {
  render(<App />);
  hello({ gameflowPhase: 'WaitingForStats' });
  act(() => { vi.advanceTimersByTime(STATS_TIMEOUT_MS - 1); });
  expect(panel().getByText('Calculando o resultado…')).toBeInTheDocument();
  act(() => { vi.advanceTimersByTime(1); });
  expect(panel().getByRole('heading', { name: 'O resultado não chegou' })).toBeInTheDocument();
  expect(panel().getByRole('button', { name: 'Jogar de novo' })).toBeEnabled();
  fireEvent.click(panel().getByRole('button', { name: 'Voltar ao início' }));
  expect(request().command).toEqual({ type: 'leavePostGame' });
});

it('avisa que o resultado não chegou no fim de partida sem estatísticas', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame' });
  expect(panel().getByRole('heading', { name: 'O resultado não chegou' })).toBeInTheDocument();
  expect(panel().getByRole('button', { name: 'Voltar ao início' })).toBeEnabled();
});

it('mostra a partida que terminou com erro, com a volta ao início e sem atalho', async () => {
  render(<App />);
  hello({ gameflowPhase: 'TerminatedInError', honorBallot: ballot });
  expect(panel().getByRole('heading', { name: 'A partida terminou com erro' })).toBeInTheDocument();
  expect(panel().getByRole('alert')).toHaveTextContent('O LoL não mandou o resultado desta partida.');
  // Sem cédula de honra numa partida que deu erro.
  expect(panel().queryByRole('region', { name: 'Honra' })).not.toBeInTheDocument();
  fireEvent.keyDown(document.body, { key: 'Enter' });
  expect(socket().send).not.toHaveBeenCalled();
  fireEvent.click(panel().getByRole('button', { name: 'Voltar ao início' }));
  expect(request().command).toEqual({ type: 'leavePostGame' });
  await respond({ status: 'error', code: 'lcu', message: 'sem lobby' });
  expect(panel().getByText('Não foi possível sair do fim de partida: sem lobby')).toBeInTheDocument();
  expect(panel().getByRole('button', { name: 'Voltar ao início' })).toBeEnabled();
});

const ballot: HonorBallot = {
  gameId: 1124,
  votes: 2,
  honoredPuuids: [],
  eligibleAllies: [
    { puuid: 'p-ksante', summonerId: 1134, championId: 897, championName: "K'Sante", role: 'TOP', summonerName: 'Rafa', bot: false },
    { puuid: 'p-viego', summonerId: 1132, championId: 234, championName: 'Viego', role: 'JUNGLE', summonerName: 'Victor', bot: false },
  ],
  eligibleOpponents: [
    { puuid: 'p-gp', summonerId: 1131, championId: 41, championName: 'Gangplank', role: 'TOP', summonerName: 'Rival', bot: false },
  ],
};
const honor = () => within(panel().getByRole('region', { name: 'Honra' }));

it('mostra as honras da partida, o seu time e o time inimigo', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  expect(honor().getByRole('img', { name: '2 de 2 honras restantes' })).toBeInTheDocument();
  expect(within(honor().getByRole('list', { name: 'Seu time' })).getAllByRole('button')).toHaveLength(2);
  expect(within(honor().getByRole('list', { name: 'Time inimigo' })).getByRole('button', { name: 'Honrar Rival' })).toBeEnabled();
  expect(honor().getAllByText('Topo')).toHaveLength(2);
  // Jogar de novo espera a votação acabar.
  expect(panel().queryByRole('button', { name: 'Jogar de novo' })).not.toBeInTheDocument();
});

it('mostra o resultado da partida e o desempenho de cada jogador na votação (#629)', () => {
  render(<App />);
  hello({ gameflowPhase: 'PreEndOfGame', honorBallot: ballot, gameResult: testGameResult });
  expect(honor().getByLabelText('Resultado da partida')).toHaveTextContent('Vitória·Ranqueada Solo/Duo·25:51');
  // Viego (p-viego) acha o jogador pelo campeão; o resultado de teste tem outros puuids.
  const viego = within(honor().getByRole('button', { name: 'Honrar Victor' }));
  expect(viego.getByText('11 / 2 / 9')).toBeInTheDocument();
  expect(viego.getByText('17,1 mil de dano')).toBeInTheDocument();
  expect(viego.getByText('13,8 mil de ouro')).toBeInTheDocument();
});

it('vota sem a faixa e sem os números quando o resultado é de outra partida', () => {
  render(<App />);
  hello({ gameflowPhase: 'PreEndOfGame', honorBallot: ballot, gameResult: { ...testGameResult, gameId: 1 } });
  expect(honor().queryByLabelText('Resultado da partida')).not.toBeInTheDocument();
  expect(honor().queryByText('11 / 2 / 9')).not.toBeInTheDocument();
});

it('honra vários jogadores, inclusive do time inimigo, até as honras acabarem', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.click(honor().getByRole('button', { name: 'Honrar Victor' }));
  expect(request().command).toEqual({ type: 'honorPlayer', puuid: 'p-viego' });
  expect(honor().getByRole('button', { name: 'Honrar Rafa' })).toBeDisabled();
  await respond();
  // A LCU aceitou, mas o voto só vale quando a cédula traz o jogador.
  expect(honor().getByRole('status')).toHaveTextContent('Voto enviado. Esperando a confirmação da Riot…');
  expect(honor().getByRole('button', { name: 'Honrar Rafa' })).toBeDisabled();
  act(() => socket().receive({ type: 'honorBallot', honorBallot: { ...ballot, honoredPuuids: ['p-viego'] } }));
  expect(honor().getByRole('img', { name: '1 de 2 honras restantes' })).toBeInTheDocument();
  expect(honor().getByRole('button', { name: 'Victor, honrado' })).toBeDisabled();
  expect(honor().getByRole('button', { name: 'Concluir' })).toBeEnabled();

  fireEvent.click(honor().getByRole('button', { name: 'Honrar Rival' }));
  expect(request().command).toEqual({ type: 'honorPlayer', puuid: 'p-gp' });
  await respond();
  act(() => socket().receive({ type: 'honorBallot', honorBallot: { ...ballot, honoredPuuids: ['p-viego', 'p-gp'] } }));
  expect(panel().queryByRole('region', { name: 'Honra' })).not.toBeInTheDocument();
  expect(panel().getByRole('status')).toHaveTextContent('Você honrou Victor e Rival.');
  expect(panel().getByRole('button', { name: 'Jogar de novo' })).toBeEnabled();
});

it('avisa quando a Riot não confirma o voto e deixa tentar de novo', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.click(honor().getByRole('button', { name: 'Honrar Rafa' }));
  await respond();
  act(() => { vi.advanceTimersByTime(HONOR_CONFIRM_MS); });
  expect(honor().getByRole('alert')).toHaveTextContent('A Riot não confirmou o voto em Rafa.');
  expect(honor().getByRole('button', { name: 'Honrar Rafa' })).toBeEnabled();
});

it('pula a honra e mostra o jogar de novo', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.click(honor().getByRole('button', { name: 'Pular' }));
  expect(request().command).toEqual({ type: 'skipHonor' });
  await respond();
  expect(panel().getByRole('status')).toHaveTextContent('Você pulou a honra desta partida.');
  expect(panel().getByRole('button', { name: 'Jogar de novo' })).toBeEnabled();
});

it('conclui depois de um voto mesmo com honras sobrando, e mesmo se a LCU recusar', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: { ...ballot, honoredPuuids: ['p-ksante'] } });
  fireEvent.click(honor().getByRole('button', { name: 'Concluir' }));
  await respond({ status: 'error', code: 'lcu', message: 'tela já encerrada' });
  expect(panel().getByRole('status')).toHaveTextContent('Você honrou Rafa.');
});

it('mostra o erro do voto e deixa votar de novo', async () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: ballot });
  fireEvent.click(honor().getByRole('button', { name: 'Honrar Rafa' }));
  await respond({ status: 'error', code: 'lcu', message: 'voto recusado' });
  expect(honor().getByRole('alert')).toHaveTextContent('Não foi possível honrar Rafa: voto recusado');
  expect(honor().getByRole('button', { name: 'Honrar Rafa' })).toBeEnabled();
});

it('cédula sem honras sobrando mostra só o resultado, e a cédula fechada some', () => {
  render(<App />);
  hello({ gameflowPhase: 'EndOfGame', honorBallot: { ...ballot, votes: 1, honoredPuuids: ['p-ksante'] } });
  expect(panel().getByRole('status')).toHaveTextContent('Você honrou Rafa.');
  act(() => socket().receive({ type: 'honorBallot', honorBallot: null }));
  expect(panel().queryByRole('status')).not.toBeInTheDocument();
  expect(panel().queryByRole('region', { name: 'Honra' })).not.toBeInTheDocument();
});

import type { ConnectionSnapshot } from '../agent/client';
import type { GameflowPhase } from '../bindings/GameflowPhase';

export function mainScreen(connection: ConnectionSnapshot) {
  const state = connection.state;
  if (connection.status !== 'connected' || state?.lcu.state !== 'connected' || !state.summoner) return 'home';
  const phase = state.gameflowPhase;
  // Fases posteriores têm prioridade sobre snapshots atrasados de fila/lobby.
  if (phase && !['None', 'Lobby', 'Matchmaking', 'ReadyCheck'].includes(phase)) return 'other';
  if (phase === 'ReadyCheck' || state.readyCheck || state.matchmaking?.searchState === 'Found') return 'found';
  if (phase === 'Matchmaking' || state.matchmaking) return 'queue';
  if (phase === 'Lobby' || state.lobby) return 'lobby';
  return 'home';
}

export function phaseLabel(phase: GameflowPhase | null | undefined): string {
  switch (phase) {
    case 'ChampSelect': return 'Seleção de campeões em andamento.';
    case 'GameStart': return 'A partida está abrindo.';
    case 'InProgress': return 'Partida em andamento.';
    case 'Reconnect': return 'Reconecte à partida.';
    case 'FailedToLaunch': return 'Não foi possível abrir a partida.';
    case 'TerminatedInError': return 'A partida terminou com erro.';
    case 'CheckedIntoTournament': return 'Inscrito no torneio.';
    case 'WaitingForStats': return 'Aguardando estatísticas da partida.';
    case 'PreEndOfGame': case 'EndOfGame': return 'Partida encerrada.';
    default: return 'Aguardando uma fase compatível com esta interface.';
  }
}

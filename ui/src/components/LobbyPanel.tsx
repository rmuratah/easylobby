import { useEffect, useId, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { isQueue, queueNames } from '../agent/queues';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import { isPosition, positionNames, validPositions } from '../agent/positions';
import type { Position } from '../bindings/Position';
import type { Queue } from '../bindings/Queue';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type LobbyCommand = Extract<Command, { type: 'createLobby' | 'leaveLobby' | 'setPositions' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
const buttonClass = 'rounded border border-sky-400 px-4 py-2 disabled:cursor-not-allowed disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-4';

export function LobbyPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <section aria-label="Lobby" className="w-full max-w-lg rounded border border-slate-700 p-4">
        <h2 className="text-xl font-semibold">Lobby</h2>
        <p>Aguarde a conexão com o agente e o League of Legends. Confira o lobby após reconectar antes de tentar novamente.</p>
      </section>
    );
  }
  // Trocar a sessão ou o jogador descarta feedback e respostas da sessão anterior.
  return <LobbyActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} />;
}

function LobbyActions({ state, send }: { state: AgentState; send: AgentClient['send'] }) {
  const { lobby, summoner, gameflowPhase: phase } = state;
  const localMember = summoner?.puuid ? lobby?.members.find(member => member.puuid === summoner.puuid) : undefined;
  const first = localMember?.firstPosition ?? 'unselected';
  const second = localMember?.secondPosition ?? 'unselected';
  const confirmedKey = `${lobby?.queueId}:${lobby?.showPositionSelector}:${localMember?.puuid}:${first}:${second}`;
  const [positions, setPositions] = useState({ confirmedKey, first, second });
  const [acceptedPositions, setAcceptedPositions] = useState<typeof positions | null>(null);
  if (positions.confirmedKey !== confirmedKey) {
    setPositions({ confirmedKey, first, second });
    setAcceptedPositions(null);
  }
  const queueId = lobby?.queueId ?? null;
  const [selection, setSelection] = useState<{ queueId: number | null; queue: Queue }>({ queueId, queue: lobby?.queue ?? 'normalDraft' });
  if (selection.queueId !== queueId) {
    setSelection({ queueId, queue: lobby?.queue ?? 'normalDraft' });
  }
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const descriptionId = useId();
  const selectId = useId();
  const firstId = useId();
  const secondId = useId();
  const positionContext = useRef(0);
  const confirmedRevision = useRef(0);
  useEffect(() => { confirmedRevision.current += 1; }, [confirmedKey]);
  useEffect(() => {
    positionContext.current += 1;
  }, [queueId, lobby?.showPositionSelector, localMember?.puuid]);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const phaseAllowsChange = phase === 'None' || phase === 'Lobby';
  const canCreate = !!summoner && phaseAllowsChange && (!lobby || lobby.isLeader);
  const canLeave = !!summoner && !!lobby && phase === 'Lobby';
  const canSetPositions = !!localMember && !!lobby?.showPositionSelector && phase === 'Lobby';
  const positionsChanged = (positions.first !== first || positions.second !== second) &&
    !(acceptedPositions?.confirmedKey === confirmedKey && acceptedPositions.first === positions.first && acceptedPositions.second === positions.second);
  const pending = feedback.status === 'pending';
  const localName = summoner?.gameName
    ? `${summoner.gameName}${summoner.tagLine ? `#${summoner.tagLine}` : ''}`
    : summoner?.displayName || 'Você';
  const restriction = !summoner ? 'Aguarde o login no League of Legends.'
    : !phaseAllowsChange ? 'Criar, trocar fila e sair do lobby só estão disponíveis fora da busca e da partida. Aguarde a fase de lobby.'
    : lobby && !lobby.isLeader ? 'Somente o líder pode trocar a fila. Você pode sair do lobby.'
    : 'Escolha uma fila para criar um lobby ou trocar a fila atual.';

  async function execute(command: LobbyCommand) {
    if (inFlight.current || (command.type === 'createLobby'
      ? !canCreate || command.queue === lobby?.queue
      : command.type === 'leaveLobby' ? !canLeave
      : !canSetPositions || !positionsChanged || !validPositions(command.first, command.second))) return;
    const context = positionContext.current;
    const revision = confirmedRevision.current;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: command.type === 'setPositions' ? 'Enviando cargos…' : command.type === 'leaveLobby' ? 'Saindo do lobby…'
      : lobby ? 'Trocando fila…' : 'Criando lobby…' });
    try {
      const result = await send(command);
      if (!mounted.current) return;
      if (command.type === 'setPositions' && context !== positionContext.current) {
        setFeedback({ status: 'idle', message: '' });
        return;
      }
      if (result.status === 'ok') {
        // Evita repetir um comando aceito enquanto o evento ainda não chegou.
        if (command.type === 'setPositions' && revision === confirmedRevision.current) {
          setAcceptedPositions({ confirmedKey, first: command.first, second: command.second });
        }
        setFeedback({ status: 'success', message: command.type === 'setPositions'
          ? 'Comando de cargos aceito. Confira as posições confirmadas pelo agente.' : command.type === 'leaveLobby'
          ? 'Saída do lobby confirmada.' : 'Lobby atualizado pelo agente.' });
      } else {
        setFeedback({ status: 'error', message: result.code === 'notConnected'
          ? 'O League of Legends desconectou. Aguarde a reconexão e confira o lobby antes de tentar novamente.'
          : `Não foi possível alterar o lobby: ${result.message}` });
      }
    } catch {
      if (!mounted.current) return;
      setFeedback({ status: 'error', message: 'Não foi possível confirmar o comando. Confira a conexão e o lobby antes de tentar novamente.' });
    } finally {
      inFlight.current = false;
    }
  }

  return (
    <section aria-label="Lobby" className="flex w-full max-w-lg flex-col gap-3 rounded border border-slate-700 p-4">
      <h2 className="text-xl font-semibold">Lobby</h2>
      {lobby ? (
        <>
          <p>Fila atual: <strong>{lobby.queue ? queueNames[lobby.queue] : `Outra fila (ID ${lobby.queueId})`}</strong></p>
          <p>{lobby.members.length} / {lobby.maxLobbySize} jogadores · {lobby.isLeader ? 'Você é o líder' : 'Você é membro'}</p>
          <ul aria-label="Jogadores no lobby">
            {lobby.members.map(member => (
              <li key={member.puuid} className="break-words">
                {member.puuid === summoner?.puuid
                  ? localName
                  : `Invocador ${member.summonerId}`}
                {' · Nível '}{member.summonerLevel}{member.isLeader ? ' · Líder' : ''}
                {lobby.showPositionSelector && (
                  <span className="block text-sm text-slate-300">
                    Primário: {positionNames[member.firstPosition]} · Secundário: {positionNames[member.secondPosition]}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </>
      ) : <p>Você não está em um lobby.</p>}
      {lobby && (lobby.showPositionSelector ? (
        <fieldset className="flex flex-col gap-2" disabled={!canSetPositions || pending}>
          <legend className="font-semibold">Seus cargos</legend>
          <p className="text-sm text-slate-300">As posições na lista de jogadores são confirmadas pelo agente.</p>
          {!localMember && <p>Aguarde a identificação do seu membro no lobby.</p>}
          {phase !== 'Lobby' && <p>A escolha de cargos está disponível na fase de lobby.</p>}
          <label htmlFor={firstId}>Cargo primário</label>
          <select id={firstId} value={positions.first} className="rounded bg-slate-900 p-2"
            onChange={event => {
              if (!isPosition(event.target.value)) return;
              const next = event.target.value;
              setPositions({ ...positions, first: next,
                second: validPositions(next, positions.second) ? positions.second : 'unselected' });
            }}>
            {(Object.keys(positionNames) as Position[]).map(position =>
              <option key={position} value={position}>{positionNames[position]}</option>)}
          </select>
          <label htmlFor={secondId}>Cargo secundário</label>
          <select id={secondId} value={positions.second} className="rounded bg-slate-900 p-2"
            disabled={!canSetPositions || pending || positions.first === 'fill' || positions.first === 'unselected'}
            onChange={event => {
              if (isPosition(event.target.value)) setPositions({ ...positions, second: event.target.value });
            }}>
            {(Object.keys(positionNames) as Position[]).map(position =>
              <option key={position} value={position} disabled={!validPositions(positions.first, position)}>{positionNames[position]}</option>)}
          </select>
          <button type="button" className={buttonClass} aria-busy={pending}
            disabled={!canSetPositions || pending || !positionsChanged || !validPositions(positions.first, positions.second)}
            onClick={() => { void execute({ type: 'setPositions', first: positions.first, second: positions.second }); }}>
            Salvar cargos
          </button>
        </fieldset>
      ) : <p>Esta fila não permite escolher cargos.</p>)}
      <p id={descriptionId} className="text-sm text-slate-300">{restriction}</p>
      <label htmlFor={selectId}>Fila</label>
      <select
        id={selectId}
        value={selection.queue}
        disabled={!canCreate || pending}
        aria-describedby={descriptionId}
        onChange={event => {
          if (isQueue(event.target.value)) setSelection({ queueId, queue: event.target.value });
        }}
        className="rounded border border-slate-600 bg-slate-900 p-2 disabled:opacity-50"
      >
        {(Object.keys(queueNames) as Queue[]).map(queue => <option key={queue} value={queue}>{queueNames[queue]}</option>)}
      </select>
      <div className="flex flex-wrap gap-3">
        <button
          type="button" className={buttonClass} aria-describedby={descriptionId} aria-busy={pending}
          disabled={!canCreate || pending || selection.queue === lobby?.queue}
          onClick={() => { void execute({ type: 'createLobby', queue: selection.queue }); }}
        >{lobby ? 'Trocar fila' : 'Criar lobby'}</button>
        <button
          type="button" className={buttonClass} aria-describedby={descriptionId} aria-busy={pending}
          disabled={!canLeave || pending}
          onClick={() => { void execute({ type: 'leaveLobby' }); }}
        >Sair do lobby</button>
      </div>
      {feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}>{feedback.message}</p>}
    </section>
  );
}
